use std::env;
use std::io::{self, Read};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Lire le prompt depuis les arguments CLI ou l'entrée standard (stdin)
    let args: Vec<String> = env::args().collect();
    let mut prompt = String::new();

    if args.len() > 1 {
        prompt = args[1..].join(" ");
    } else {
        // Lecture depuis stdin (utile pour les pipes : cat file.rs | ask_local)
        io::stdin().read_to_string(&mut prompt)?;
    }

    if prompt.trim().is_empty() {
        eprintln!("Usage: ask_local <prompt>");
        eprintln!("   Ou: echo '<prompt>' | ask_local");
        std::process::exit(1);
    }

    // 2. Configuration de l'URL LM Studio (avec fallback sur le port par défaut)
    let url = env::var("LM_STUDIO_URL")
        .unwrap_or_else(|_| "http://localhost:1234/v1/chat/completions".to_string());
    
    // LM Studio ignore généralement ce champ s'il n'y a qu'un modèle chargé, mais c'est requis par l'API
    let model = env::var("LM_STUDIO_MODEL")
        .unwrap_or_else(|_| "local-model".to_string());

    // 3. Construction du payload compatible OpenAI
    let payload = json!({
        "model": model,
        "messages": [
            { "role": "user", "content": prompt }
        ],
        "temperature": 0.3
    });

    // 4. Appel HTTP synchrone (ureq)
    let response = ureq::post(&url)
        .set("Content-Type", "application/json")
        .send_json(payload);

    match response {
        Ok(res) => {
            let response_json: serde_json::Value = res.into_json()?;
            if let Some(content) = response_json["choices"][0]["message"]["content"].as_str() {
                println!("{}", content);
            } else {
                eprintln!("Erreur de parsing de la réponse: {}", response_json);
                std::process::exit(1);
            }
        },
        Err(ureq::Error::Status(code, response)) => {
            eprintln!("Erreur API (Code {}): {:?}", code, response.into_string());
            std::process::exit(1);
        },
        Err(e) => {
            eprintln!("Impossible de contacter LM Studio: {}. Le serveur tourne-t-il sur {} ?", e, url);
            std::process::exit(1);
        }
    }

    Ok(())
}
