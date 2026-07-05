fn main() {
    let host = std::env::var("TARGET_HOST").unwrap_or_else(|_| "example.com".to_string());
    println!("Requesting: {}", host);
    
    let headers = wasi::http::types::Headers::new();
    let req = wasi::http::types::OutgoingRequest::new(headers);
    req.set_method(&wasi::http::types::Method::Get).unwrap();
    req.set_scheme(Some(&wasi::http::types::Scheme::Https)).unwrap();
    req.set_authority(Some(&host)).unwrap();
    req.set_path_with_query(Some("/")).unwrap();
    
    match wasi::http::outgoing_handler::handle(req, None) {
        Ok(_) => println!("Success!"),
        Err(_) => println!("Error!"),
    }
}
