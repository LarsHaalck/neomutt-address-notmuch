use ini::Ini;
use std::collections::HashMap;
use std::path::PathBuf;
use structopt::StructOpt;

// A basic example
#[derive(StructOpt, Debug)]
#[structopt()]
struct Opt {
    // config file
    #[structopt(short, long, parse(from_os_str))]
    config: Option<PathBuf>,

    // search string
    #[structopt(name = "QUERY")]
    name: String,
}

#[derive(Debug)]
struct MailEntry {
    count: i32,
    display_name: String, // can be empty
}

fn generate_query_string(
    db: &notmuch::Database,
    all_mails: Vec<&str>,
    name: &str,
) -> Result<Vec<String>, notmuch::Error> {
    let mut query_strings: Vec<String> = Vec::new();

    let from_all_mails: Vec<_> = all_mails
        .iter()
        .map(|mail| format!("from:{}", mail))
        .collect();
    let mut query_string = from_all_mails.join(" or ");
    query_string = format!("({}) and to:{}", query_string, name);
    let query = db.create_query(&query_string)?;
    let count = query.count_messages()?;
    query_strings.push(query_string);

    if count < 10 {
        let query_string = format!("from:{}", name);
        query_strings.push(query_string);
    }

    Ok(query_strings)
}

fn contains_any(query: &str, tests: &Vec<&str>) -> bool {
    let mut res = true;
    for test in tests {
        if !query.contains(&test.to_lowercase()) {
            res = false;
            break;
        }
    }
    res
}

fn retrieve_mail_entries (
    mails: Vec<String>,
    name: &str,
) -> Result<HashMap<String, MailEntry>, regex::Error> {
    let email_regex = regex::Regex::new(
        r"([a-zA-Z0-9_+]([a-zA-Z0-9_+.]*[a-zA-Z0-9_+])?)@([a-zA-Z0-9]+([\-\.]{1}[a-zA-Z0-9]+)*\.[a-zA-Z]{2,6})",
    )?;
    let display_regex = regex::Regex::new(r"(^[a-z0-9A-Z ,öäüÖÄÜß\-_\.]*)<")?;
    let mut map: HashMap<String, MailEntry> = HashMap::new();
    // println!("{:?}", mails);
    for mail in mails {
        // println!("{:?}", mail);
        let capture = match email_regex.captures(&mail) {
            Some(capture) => capture,
            None => continue,
        };
        let email_stripped = match capture.get(0) {
            Some(match_group) => match_group.as_str().to_lowercase(),
            None => continue,
        };

        let mut display_stripped = "";
        if display_regex.is_match(&mail) {
            let capture = match display_regex.captures(&mail) {
                Some(capture) => capture,
                None => continue,
            };
            display_stripped = match capture.get(1) {
                Some(match_group) => match_group.as_str().trim(),
                None => "",
            };
        }

        let tests: Vec<&str> = name.split(" ").collect();
        // println!("{:?}", email_stripped);
        // println!("{:?}\n", display_stripped);
        if (contains_any(&email_stripped, &tests)
            || contains_any(&display_stripped.to_lowercase(), &tests))
            && !email_stripped.contains("noreply")
        {
            match map.get_mut(&email_stripped) {
                Some(entry) => {
                    if display_stripped.len() > entry.display_name.len() {
                        entry.display_name = display_stripped.to_string();
                    }
                    entry.count += 1;
                }
                None => {
                    let entry = MailEntry {
                        count: 1,
                        display_name: display_stripped.to_string(),
                    };
                    map.insert(email_stripped, entry);
                }
            }
        }
    }
    Ok(map)
}

fn sort_by_count(map : HashMap<String, MailEntry>) {
    let mut entry_list : Vec<(&String, &MailEntry)> = map.iter().collect();
    entry_list.sort_by(|a, b| (b.1.count).cmp(&a.1.count));

    for entry in entry_list {
        if entry.1.display_name.is_empty() {
            println!("{}", entry.0);
        } else {
            println!("{} <{}>", entry.1.display_name, entry.0);
        }
    }
}

fn run_queries(
    db: &notmuch::Database,
    query_strings: Vec<String>,
) -> Result<Vec<String>, notmuch::Error> {
    let header_fields = vec![vec!["to", "cc", "bcc"], vec!["from"]];
    let mut collected_mails: Vec<String> = Vec::new();
    for (i, query_string) in query_strings.iter().enumerate() {
        let query = db.create_query(&query_string)?;
        let messages = query.search_messages()?;

        for message in messages {
            for header_field in &header_fields[i] {
                let header = message.header(header_field)?;
                let mut mails: String = match header {
                    Some(header) => header.to_string(),
                    None => continue,
                };

                println!("{:?}", mails);
                mails = mails.replace(&['\"', '\\', '\t'][..], "");
                let mails: Vec<String> = mails
                    .split(",")
                    .filter(|mail| !mail.is_empty())
                    .map(|mail| mail.to_string())
                    .collect();
                collected_mails.extend(mails);
            }
        }
    }
    Ok(collected_mails)
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
            default_path.push(".notmuch-config");
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
    let queries = match generate_query_string(&db, all_mails, &opt.name) {
        Ok(queries) => queries,
        Err(e) => {
            println!("Error creating queries: {}", e);
            return;
        }
    };
    let query_results = match run_queries(&db, queries) {
        Ok(res) => res,
        Err(e) => {
            println!("Error running queries: {}", e);
            return;
        }
    };
    let set = match retrieve_mail_entries(query_results, &opt.name) {
        Ok(set) => set,
        Err(e) => {
            println!("Error retrieving mails from queries: {}", e);
            return;
        }
    };
    let _blubb = sort_by_count(set);



}
