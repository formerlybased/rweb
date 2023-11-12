use clap::{Parser, Subcommand};

use tokio::fs::File;
use tokio::fs;
use tokio::sync::futures;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use tokio_util::codec::{BytesCodec, FramedRead};

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Result, Server, StatusCode};

#[derive(Parser)]
#[command()]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    New { name: Option<String> },
    Run,
}

#[derive(Clone)]
struct View {
    web_path: String,
    source: String,
}


impl View {
    async fn build(path: String) -> View {
        let web_path: String;

        if path.strip_prefix("./pages") == None {
            web_path = format!("/{}", path.strip_suffix(".html").unwrap().to_string());
        } else {
            web_path = format!("{}", path.strip_prefix("./pages").unwrap().strip_suffix(".html").unwrap().to_string());
        }
        let content = fs::read(path).await;
        let source = generate_view(String::from_utf8(content.unwrap()).unwrap(), String::from("")); // TODO:
        // add proper header and footer stuff

        View {
            web_path,
            source,
        }
    }

}

fn generate_view(src: String, header: String) -> String {
    return format!("
    <html>\n
    <head>\n
    <title>Website</title>\n
    </head>\n
    <body>\n
    {header}\n
    {src}\n
    </body>\n
    </html>
    ");
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Run) => {
            run_server();
        }
        Some(Commands::New { name }) => {
            create_project(name.to_owned());
        }
        None => {
            println!("Usage: rweb [command]\nUse --help for help");
        }
    }
}

#[tokio::main]
async fn run_server() {
    let views: Vec<View> = make_views().await.unwrap();

    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

    let service = make_service_fn(move |_| { 
        let views = views.clone();
        async move {Ok::<_, hyper::Error>(service_fn(move |_req| {
            let views = views.clone();
            async move { respond(_req, views).await }
        }))}
    });

    let server = Server::bind(&address).serve(service);
    let server = server.with_graceful_shutdown(shutdown_signal_await());

    println!("Serving on http://{}", address);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }

    println!("\nServer shutdown...");
}

async fn make_views() -> std::io::Result<Vec<View>> {
    let mut views: Vec<View> = Vec::new();
    let home_view = View::build(String::from("home.html")).await;
    views.push(home_view);
    let error_view = View::build(String::from("404.html")).await;
    views.push(error_view);

    for f in std::fs::read_dir("./pages")? {
        let file_path = f?.path();
        if file_path.is_dir() {
            break;
        }
        let file_path = file_path.into_os_string().into_string().unwrap();
        views.push(View::build(file_path).await);
    }

    Ok(views)
}

async fn respond(req: Request<Body>, views: Vec<View> ) -> Result<Response<Body>> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => send_view(&views[0]).await,
        (&Method::GET, path) => Ok(route(&views, String::from(path)).await),
        _ => Ok(handle_404(&views[1])),
    }
}

async fn route(views: &Vec<View>, path: String) -> Response<Body> {
    for v in views {
        if v.web_path == path { return send_view(&v).await.unwrap() };
    }

    return send_view(&views[1]).await.unwrap()
}

fn handle_404(view: &View) -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(view.clone().source.into())
        .unwrap()
}

async fn send_view(v: &View) -> Result<Response<Body>> {
    return Ok(Response::builder().status(StatusCode::FOUND).body(v.clone().source.into()).unwrap());
}

fn create_project(dir: Option<String>) {
    if dir == None {
        println!("Usage: rweb new <directory>");
        return
    }
    let dir: String = dir.unwrap();
    std::fs::create_dir(dir.clone()).unwrap();
    std::fs::write(dir.clone() + "/home.html", "<h1>Home page!</h1>").unwrap();
    std::fs::write(dir.clone() + "/404.html", "<h1>404 Page!</h1>").unwrap();
    std::fs::create_dir(dir.clone() + "/pages/").unwrap();
}

async fn shutdown_signal_await() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to attach ctrl-c handler");
}
