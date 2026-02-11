"""Jellyfin media server client."""

import logging
from typing import Any

import requests

from submate.config import Config

logger = logging.getLogger(__name__)


class JellyfinClient:
    """Client for interacting with Jellyfin Media Server."""

    def __init__(self, config: Config) -> None:
        """Initialize Jellyfin client.

        Args:
            config: Application configuration
        """
        self.config = config
        self.server_url: str | None = None
        self.api_key: str | None = None
        self._admin_user_id: str | None = None

    def is_configured(self) -> bool:
        """Check if Jellyfin is configured.

        Returns:
            True if server URL and API key are set
        """
        return bool(self.config.jellyfin.server_url and self.config.jellyfin.api_key)

    def connect(self) -> None:
        """Connect to the Jellyfin server.

        Raises:
            RuntimeError: If Jellyfin is not configured or connection fails
        """
        if not self.is_configured():
            raise RuntimeError("Jellyfin not configured")

        logger.info("Connecting to Jellyfin server: %s", self.config.jellyfin.server_url)

        try:
            self.server_url = self.config.jellyfin.server_url
            self.api_key = self.config.jellyfin.api_key

            # Test connection by fetching library sections
            headers = {"X-MediaBrowser-Token": self.api_key}
            response = requests.get(
                f"{self.server_url}/Library/VirtualFolders",
                headers=headers,
                timeout=10,
            )
            response.raise_for_status()

            logger.info("Connected to Jellyfin successfully")
        except Exception as e:
            logger.error("Failed to connect to Jellyfin: %s", e, exc_info=True)
            raise

    def _get_admin_user_id(self) -> str:
        """Get admin user ID (cached).

        Returns:
            Admin user ID

        Raises:
            RuntimeError: If no admin found
        """
        if hasattr(self, "_admin_user_id") and self._admin_user_id:
            return self._admin_user_id

        if not self.server_url or not self.api_key:
            raise RuntimeError("Not connected to Jellyfin server")

        headers = {"Authorization": f"MediaBrowser Token={self.api_key}"}
        response = requests.get(
            f"{self.server_url}/Users",
            headers=headers,
            timeout=10,
        )
        response.raise_for_status()

        users = response.json()
        for user in users:
            if user.get("Policy", {}).get("IsAdministrator"):
                self._admin_user_id = user["Id"]
                return self._admin_user_id

        raise RuntimeError("No admin user found in Jellyfin")

    def get_file_path(self, item_id: str) -> str:
        """Get file path for a media item.

        Args:
            item_id: Jellyfin item ID

        Returns:
            Full file path to media file

        Raises:
            RuntimeError: If not configured or API call fails
        """
        if not self.server_url or not self.api_key:
            raise RuntimeError("Not connected to Jellyfin server")

        admin_id = self._get_admin_user_id()
        headers = {"Authorization": f"MediaBrowser Token={self.api_key}"}

        try:
            url = f"{self.server_url}/Users/{admin_id}/Items/{item_id}"
            response = requests.get(url, headers=headers, timeout=10)
            response.raise_for_status()

            data = response.json()
            file_path = data.get("Path")

            if not file_path:
                raise RuntimeError(f"No file path found for item {item_id}")

            logger.debug("Retrieved file path for %s: %s", item_id, file_path)
            return str(file_path)

        except requests.exceptions.RequestException as e:
            logger.error("Failed to get file path: %s", e, exc_info=True)
            raise RuntimeError(f"Jellyfin API error: {e}") from e

    def refresh_item(self, item_id: str) -> None:
        """Refresh metadata for a specific item.

        Args:
            item_id: Jellyfin item ID

        Raises:
            RuntimeError: If not configured or API call fails
        """
        if not self.server_url or not self.api_key:
            raise RuntimeError("Not connected to Jellyfin server")

        url = f"{self.server_url}/Items/{item_id}/Refresh"
        headers = {"Authorization": f"MediaBrowser Token={self.api_key}"}

        try:
            response = requests.post(
                url,
                headers=headers,
                params={"Recursive": "true"},
                timeout=10,
            )
            response.raise_for_status()
            logger.debug("Refreshed metadata for item %s", item_id)
        except requests.exceptions.RequestException as e:
            logger.error("Failed to refresh item: %s", e, exc_info=True)
            raise RuntimeError(f"Jellyfin API error: {e}") from e

    def refresh_library(self, library_name: str) -> None:
        """Refresh a specific Jellyfin library.

        Args:
            library_name: Name of the library to refresh

        Raises:
            RuntimeError: If not connected to server
        """
        if self.server_url is None or self.api_key is None:
            raise RuntimeError("Not connected to Jellyfin server")

        logger.info("Refreshing Jellyfin library: %s", library_name)

        try:
            headers = {"X-MediaBrowser-Token": self.api_key}

            # Get all libraries
            response = requests.get(
                f"{self.server_url}/Library/VirtualFolders",
                headers=headers,
                timeout=10,
            )
            response.raise_for_status()
            libraries = response.json()

            # Find the library by name
            for library in libraries:
                if library.get("Name") == library_name:
                    library_id = library.get("Id")

                    # Trigger library refresh
                    refresh_response = requests.post(
                        f"{self.server_url}/Items/{library_id}/Refresh",
                        headers=headers,
                        params={"Recursive": "true"},
                        timeout=10,
                    )
                    refresh_response.raise_for_status()

                    logger.info("Refreshed library: %s", library_name)
                    return

            logger.warning("Library not found: %s", library_name)
        except Exception as e:
            logger.error("Failed to refresh library %s: %s", library_name, e, exc_info=True)
            raise

    def refresh_all_libraries(self) -> list[str]:
        """Refresh all configured Jellyfin libraries.

        Returns:
            List of library names that were refreshed
        """
        if not self.config.jellyfin.libraries:
            logger.info("No Jellyfin libraries configured")
            return []

        refreshed = []
        for library in self.config.jellyfin.libraries:
            try:
                self.refresh_library(library)
                refreshed.append(library)
            except Exception as e:
                logger.error("Failed to refresh %s: %s", library, e, exc_info=True)

        return refreshed

    def get_libraries(self) -> list[dict[str, Any]]:
        """Get all libraries (virtual folders) from Jellyfin.

        Returns:
            List of libraries with Id, Name, CollectionType

        Raises:
            RuntimeError: If not connected to server
        """
        if not self.server_url or not self.api_key:
            raise RuntimeError("Not connected to Jellyfin server")

        headers = {"X-MediaBrowser-Token": self.api_key}
        response = requests.get(
            f"{self.server_url}/Library/VirtualFolders",
            headers=headers,
            timeout=10,
        )
        response.raise_for_status()
        result: list[dict[str, Any]] = response.json()
        return result

    def get_library_items(
        self,
        library_id: str,
        item_type: str = "Movie",
        start_index: int = 0,
        limit: int = 100,
    ) -> dict[str, Any]:
        """Get items from a library.

        Args:
            library_id: The library (parent) ID to browse
            item_type: Type of items to fetch (Movie, Series, Episode, etc.)
            start_index: Starting index for pagination
            limit: Maximum number of items to return

        Returns:
            Dict with Items list and TotalRecordCount

        Raises:
            RuntimeError: If not connected to server
        """
        if not self.server_url or not self.api_key:
            raise RuntimeError("Not connected to Jellyfin server")

        admin_id = self._get_admin_user_id()
        headers = {"Authorization": f"MediaBrowser Token={self.api_key}"}
        params = {
            "ParentId": library_id,
            "IncludeItemTypes": item_type,
            "Recursive": "true",
            "StartIndex": start_index,
            "Limit": limit,
            "Fields": "Path,Overview,PremiereDate,RunTimeTicks",
            "SortBy": "SortName",
            "SortOrder": "Ascending",
        }

        response = requests.get(
            f"{self.server_url}/Users/{admin_id}/Items",
            headers=headers,
            params=params,
            timeout=10,
        )
        response.raise_for_status()
        result: dict[str, Any] = response.json()
        return result

    def get_series_episodes(self, series_id: str) -> dict[str, Any]:
        """Get all episodes for a series.

        Args:
            series_id: The series item ID

        Returns:
            Dict with Items list and TotalRecordCount

        Raises:
            RuntimeError: If not connected to server
        """
        if not self.server_url or not self.api_key:
            raise RuntimeError("Not connected to Jellyfin server")

        headers = {"Authorization": f"MediaBrowser Token={self.api_key}"}
        response = requests.get(
            f"{self.server_url}/Shows/{series_id}/Episodes",
            headers=headers,
            timeout=10,
        )
        response.raise_for_status()
        result: dict[str, Any] = response.json()
        return result

    def get_item(self, item_id: str) -> dict[str, Any]:
        """Get details for a single item.

        Args:
            item_id: The item ID to fetch

        Returns:
            Dict with item details

        Raises:
            RuntimeError: If not connected to server
        """
        if not self.server_url or not self.api_key:
            raise RuntimeError("Not connected to Jellyfin server")

        admin_id = self._get_admin_user_id()
        headers = {"Authorization": f"MediaBrowser Token={self.api_key}"}

        response = requests.get(
            f"{self.server_url}/Users/{admin_id}/Items/{item_id}",
            headers=headers,
            timeout=10,
        )
        response.raise_for_status()
        result: dict[str, Any] = response.json()
        return result

    def get_poster_url(self, item_id: str) -> str:
        """Get the poster image URL for an item.

        Args:
            item_id: The item ID

        Returns:
            URL to the primary poster image

        Raises:
            RuntimeError: If not connected to server
        """
        if not self.server_url or not self.api_key:
            raise RuntimeError("Not connected to Jellyfin server")

        return f"{self.server_url}/Items/{item_id}/Images/Primary"
