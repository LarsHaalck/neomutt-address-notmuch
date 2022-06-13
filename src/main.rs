use ini::Ini;
use std::collections::HashMap;
use std::path::PathBuf;
use structopt::StructOpt;
extern crate notmuch;

#[derive(StructOpt)]
#[structopt()]
struct Opt {
    /// path to notmuch config file
    #[structopt(short, long, parse(from_os_str))]
    config: Option<PathBuf>,

    /// search string
    #[structopt(name = "QUERY")]
    name: String,
}

struct MailEntry {
    count: i32,                          // how often was the mail used
    display_names: HashMap<String, i32>, // how often the display name
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

fn run_queries(
    db: &notmuch::Database,
    query_strings: Vec<String>,
) -> Result<Vec<(String, String)>, notmuch::Error> {
    let mut mail_vec: Vec<(String, String)> = Vec::new();
    let header_fields = vec![vec!["to", "cc", "bcc"], vec!["from"]];
    for (i, query_string) in query_strings.iter().enumerate() {
        let query = db.create_query(&query_string)?;
        let messages = query.search_messages()?;

        for message in messages {
            for header_field in &header_fields[i] {
                let header = message.header(header_field)?;
                let header = match header {
                    Some(header) => header,
                    None => continue,
                };

                let addr_list = match mailparse::addrparse(&header) {
                    Ok(addr_list) => addr_list,
                    Err(_) => continue,
                };
                for addr in addr_list.iter() {
                    match addr {
                        mailparse::MailAddr::Single(info) => {
                            let mut display_name = info
                                .display_name
                                .as_ref()
                                .map(|s| s.as_str())
                                .unwrap_or("")
                                .replace(&['\"', '\\', '\t', '\''][..], "");
                            if display_name.contains('@') {
                                display_name.clear();
                            }
                            mail_vec.push((info.addr.clone(), display_name));
                        }
                        _ => continue,
                    }
                }
            }
        }
    }
    Ok(mail_vec)
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

fn retrieve_mail_entries(mails: Vec<(String, String)>, name: &str) -> HashMap<String, MailEntry> {
    let mut mail_map: HashMap<String, MailEntry> = HashMap::new();

    for (mail, display_name) in mails {
        let tests: Vec<&str> = name.split(" ").collect();
        if (contains_any(&mail, &tests) || contains_any(&display_name.to_lowercase(), &tests))
            && !mail.contains("reply")
        {
            // check if mail exits in map
            match mail_map.get_mut(&mail) {
                Some(mail_entry) => {
                    if !display_name.is_empty() {
                        match mail_entry.display_names.get_mut(&display_name) {
                            Some(display_entry) => *display_entry += 1,
                            None => {
                                mail_entry.display_names.insert(display_name.to_string(), 1);
                            }
                        }
                    }
                    mail_entry.count += 1;
                }
                None => {
                    let mut mail_entry = MailEntry {
                        count: 1,
                        display_names: HashMap::new(),
                    };
                    if !display_name.is_empty() {
                        mail_entry.display_names.insert(display_name.to_string(), 1);
                    }
                    mail_map.insert(mail, mail_entry);
                }
            }
        }
    }
    mail_map
}

fn sort_by_count(map: HashMap<String, MailEntry>) {
    let mut entry_list: Vec<(&String, &MailEntry)> = map.iter().collect();
    entry_list.sort_by(|a, b| (b.1.count).cmp(&a.1.count));

    println!(
        "Searching database ... {} matching entries",
        entry_list.len()
    );
    for entry in entry_list {
        if entry.1.display_names.is_empty() {
            println!("{}", entry.0);
        } else {
            let mut max_count: i32 = 0;
            let mut max_display_name: &str = "";
            for (display_name, count) in &entry.1.display_names {
                if *count > max_count {
                    max_count = *count;
                    max_display_name = display_name;
                }
            }
            println!("{}\t{}", entry.0, max_display_name);
        }
    }
}

fn get_config_path(argument: Option<PathBuf>) -> Option<PathBuf> {
    // try argument, then env, then default
    match argument {
        Some(dir) => Some(dir),
        None => match std::env::var_os("NOTMUCH_CONFIG") {
            Some(val) => Some(PathBuf::from(val)),
            None => match dirs::home_dir() {
                Some(mut dir) => {
                    dir.push(".notmuch-config");
                    Some(dir)
                }
                None => None,
            },
        },
    }
}

fn main() {
    let opt = Opt::from_args();

    // read config from default path, argument or env variable
    let config_path = match get_config_path(opt.config) {
        Some(dir) => dir,
        None => {
            println!("Could not find configuration file .notmuch-config");
            return;
        }
    };

    let config = match Ini::load_from_file(&config_path) {
        Ok(ini) => ini,
        Err(e) => {
            println!("Error loading notmuch-config file: {}", e);
            return;
        }
    };

    // fetch primary and all other mails
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

    // get database path
    let path = match config.get_from(Some("database"), "path") {
        Some(path) => path,
        None => {
            println!("Couldn't find field path in config file.");
            return;
        }
    };
    all_mails.push(&primary_email);

    let db = notmuch::Database::open_with_config(
        Some(&path),
        notmuch::DatabaseMode::ReadOnly,
        Some(config_path),
        None,
    )
    .unwrap();
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
    let map = retrieve_mail_entries(query_results, &opt.name);
    sort_by_count(map);
}
