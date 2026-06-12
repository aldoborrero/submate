//! Request-shape parity with the Python `JellyfinClient`.
//!
//! Each test stands up a `wiremock` server and asserts that a client method
//! issues exactly the request the Python implementation does: HTTP method,
//! path, the `Authorization: MediaBrowser Token=<key>` header, and any query
//! params or body. Mocks are registered with `.expect(1)`, so the server's
//! drop-time verification fails if the recorded request does not match.

mod requests {
    use submate_jellyfin::JellyfinClient;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const API_KEY: &str = "test-api-key";
    const AUTH: &str = "MediaBrowser Token=test-api-key";

    async fn connected_client(server: &MockServer, libraries: Vec<String>) -> JellyfinClient {
        // connect() probes GET /Library/VirtualFolders. `up_to_n_times(1)` so it
        // only serves the connect probe; later VirtualFolders mounts (e.g. the
        // refresh_library lookup) take over once this one is exhausted.
        Mock::given(method("GET"))
            .and(path("/Library/VirtualFolders"))
            .and(header("Authorization", AUTH))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .up_to_n_times(1)
            .mount(server)
            .await;

        let mut client = JellyfinClient::new(server.uri(), API_KEY, libraries);
        client.connect().await.expect("connect");
        client
    }

    #[tokio::test]
    async fn connect_probes_virtual_folders_with_auth() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/Library/VirtualFolders"))
            .and(header("Authorization", AUTH))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(1)
            .mount(&server)
            .await;

        let mut client = JellyfinClient::new(server.uri(), API_KEY, vec![]);
        client.connect().await.expect("connect");
        // Drop verifies the mock was hit exactly once.
    }

    #[tokio::test]
    async fn get_file_path_resolves_admin_then_item() {
        let server = MockServer::start().await;
        let mut client = connected_client(&server, vec![]).await;

        // _get_admin_user_id -> GET /Users
        Mock::given(method("GET"))
            .and(path("/Users"))
            .and(header("Authorization", AUTH))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "Id": "user-non-admin", "Policy": { "IsAdministrator": false } },
                { "Id": "admin-1", "Policy": { "IsAdministrator": true } }
            ])))
            .expect(1)
            .mount(&server)
            .await;

        // get_file_path -> GET /Users/{adminId}/Items/{itemId}
        Mock::given(method("GET"))
            .and(path("/Users/admin-1/Items/item-42"))
            .and(header("Authorization", AUTH))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "Path": "/media/movie.mkv" })),
            )
            .expect(1)
            .mount(&server)
            .await;

        let path = client.get_file_path("item-42").await.expect("get_file_path");
        assert_eq!(path, "/media/movie.mkv");
    }

    #[tokio::test]
    async fn refresh_item_posts_recursive_refresh() {
        let server = MockServer::start().await;
        let client = connected_client(&server, vec![]).await;

        // refresh_item -> POST /Items/{itemId}/Refresh?Recursive=true
        Mock::given(method("POST"))
            .and(path("/Items/item-99/Refresh"))
            .and(query_param("Recursive", "true"))
            .and(header("Authorization", AUTH))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        client.refresh_item("item-99").await.expect("refresh_item");
    }

    #[tokio::test]
    async fn refresh_library_looks_up_then_refreshes() {
        let server = MockServer::start().await;
        let client = connected_client(&server, vec![]).await;

        // refresh_library -> GET /Library/VirtualFolders (lookup by Name)
        Mock::given(method("GET"))
            .and(path("/Library/VirtualFolders"))
            .and(header("Authorization", AUTH))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "Name": "Other", "Id": "lib-other" },
                { "Name": "Movies", "Id": "lib-movies" }
            ])))
            .expect(1)
            .mount(&server)
            .await;

        // -> POST /Items/{libraryId}/Refresh?Recursive=true
        Mock::given(method("POST"))
            .and(path("/Items/lib-movies/Refresh"))
            .and(query_param("Recursive", "true"))
            .and(header("Authorization", AUTH))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        client.refresh_library("Movies").await.expect("refresh_library");
    }

    #[tokio::test]
    async fn refresh_all_libraries_refreshes_each_configured() {
        let server = MockServer::start().await;
        let libraries = vec!["Movies".to_string(), "Shows".to_string()];
        let client = connected_client(&server, libraries).await;

        // refresh_all_libraries -> refresh_library per configured library.
        // Each refresh_library re-fetches the folder list, so /Library/VirtualFolders
        // is hit once per library.
        Mock::given(method("GET"))
            .and(path("/Library/VirtualFolders"))
            .and(header("Authorization", AUTH))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "Name": "Movies", "Id": "lib-movies" },
                { "Name": "Shows", "Id": "lib-shows" }
            ])))
            .expect(2)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/Items/lib-movies/Refresh"))
            .and(query_param("Recursive", "true"))
            .and(header("Authorization", AUTH))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/Items/lib-shows/Refresh"))
            .and(query_param("Recursive", "true"))
            .and(header("Authorization", AUTH))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let refreshed = client.refresh_all_libraries().await;
        assert_eq!(refreshed, vec!["Movies".to_string(), "Shows".to_string()]);
    }
}
