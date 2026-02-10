"""Server command for webhook integration."""

import click

from submate.config import get_config

from ..utils import console


@click.command()
@click.option("--host", "-h", default="0.0.0.0", help="Host to bind to")
@click.option("--port", "-p", type=int, help="Port to run webhook server on")
@click.pass_context
def server(ctx: click.Context, port: int | None, host: str) -> None:
    """Start the webhook server for Jellyfin integration.

    Accepts webhooks from Jellyfin and automatically queues media
    for subtitle generation. Run a worker separately to process queue.

    Examples:
        submate server
        submate server --port 9000 --host 0.0.0.0
    """
    import uvicorn

    config_file = ctx.obj.get("config_file")
    config = get_config(config_file)
    server_port = port or config.server.port

    console.print("[cyan]Starting Submate webhook server...[/cyan]")
    console.print(f"  Host: {host}")
    console.print(f"  Port: {server_port}")
    console.print("\n[yellow]Note:[/yellow] Start worker separately: submate worker\n")

    uvicorn.run(
        "submate.server:app",
        host=host,
        port=server_port,
        reload=False,
        log_level="info",
    )
