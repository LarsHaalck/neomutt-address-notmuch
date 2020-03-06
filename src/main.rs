use ini::Ini;
use notmuch::Error;
use std::path::PathBuf;
use structopt::StructOpt;

/// A basic example
#[derive(StructOpt, Debug)]
#[structopt()]
struct Opt {
    /// Output file
    #[structopt(short, long, parse(from_os_str))]
    config: Option<PathBuf>,

    /// search string
    #[structopt(name = "QUERY")]
    name: String,
}

fn generate_query_string(
    db: notmuch::Database,
    all_mails: Vec<&str>,
    name: &str,
) -> Result<Vec<String>, Error> {
    let mut query_strings: Vec<String> = Vec::new();

    let from_all_mails: Vec<_> = all_mails
        .iter()
        .map(|mail| format!("from: {}", mail))
        .collect();
    let mut query_string = from_all_mails.join(" or ");
    query_string = format!("({}) and to: {}", query_string, name);

    let query = db.create_query(&query_string)?;
    let count = query.count_messages()?;

    query_strings.push(query_string);
    if count < 10 {
        return Ok(query_strings);
    }

    let query_string = format!("from: {}", name);
    query_strings.push(query_string);
    Ok(query_strings)
}

// go through all headers, split on commas, match occurences and sort them descending

fn main() {
    let opt = Opt::from_args();

    // Load the user's config
    let config = match opt.config {
        Some(dir) => dir,
        None => {
            println!("Using default location ~/.notmuch-config");
            let mut default_path = match dirs::home_dir() {
                Some(dir) => dir,
                None => {
                    println!("Could not find configuration file neomutt_address.toml");
                    return;
                }
            };
            default_path.push(".neomutt-config");
            default_path
        }
    };

    let config = match Ini::load_from_file(config) {
        Ok(ini) => ini,
        Err(e) => {
            println!("Error config file: {}", e);
            return;
        }
    };

    let primary_email = match config.get_from(Some("user"), "primary_email") {
        Some(primary_email) => primary_email,
        None => {
            println!("Couldn't find field primary_email in config file.");
            return;
        }
    };

    let mut all_mails: Vec<&str> = match config.get_from(Some("user"), "other_email") {
        Some(other_email) => other_email
            .split(";")
            .filter(|mail| !mail.is_empty())
            .collect(),
        None => Vec::new(),
    };

    let path = match config.get_from(Some("database"), "path") {
        Some(path) => path,
        None => {
            println!("Couldn't find field path in config file.");
            return;
        }
    };

    all_mails.push(&primary_email);
    let db = notmuch::Database::open(&path, notmuch::DatabaseMode::ReadOnly).unwrap();
    let _queries = generate_query_string(db, all_mails, &opt.name);
}
