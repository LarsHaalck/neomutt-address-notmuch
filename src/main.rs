use ini::Ini;
use std::collections::HashMap;
use std::path::PathBuf;
use structopt::StructOpt;

// TODO: env variable for notmuch config

#[derive(StructOpt)]
#[structopt()]
struct Opt {
    // config file
    #[structopt(short, long, parse(from_os_str))]
    config: Option<PathBuf>,

    // search string
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

fn retrieve_mail_entries(
    mails: String,
    name: &str,
) -> Result<HashMap<String, MailEntry>, regex::Error> {
    let mut mail_map: HashMap<String, MailEntry> = HashMap::new();

    let email_regex = regex::Regex::new(
        r"([a-zA-Z0-9_+]([a-zA-Z0-9_+\-.]*[a-zA-Z0-9_+])?)@([a-zA-Z0-9]+([\-\.]{1}[a-zA-Z0-9]+)*\.[a-zA-Z]{2,6})",
    )?;
    let matches: Vec<_> = email_regex.captures_iter(&mails).collect();
    let mut last_end = 0;
    for i in 0..matches.len() {
        let email_capture = match matches[i].get(0) {
            Some(match_group) => match_group,
            None => continue,
        };

        let display_name_stripped = mails[last_end..email_capture.start()]
            .trim_matches(|c| "<>,".contains(c) || char::is_whitespace(c));
        last_end = email_capture.end();

        let email_stripped = email_capture.as_str().to_lowercase();

        let tests: Vec<&str> = name.split(" ").collect();
        if (contains_any(&email_stripped, &tests)
            || contains_any(&display_name_stripped.to_lowercase(), &tests))
            && !email_stripped.contains("reply")
        {
            // check if mail exits in map
            match mail_map.get_mut(&email_stripped) {
                Some(mail_entry) => {
                    if !display_name_stripped.is_empty() {
                        match mail_entry.display_names.get_mut(display_name_stripped) {
                            Some(display_entry) => *display_entry += 1,
                            None => {
                                mail_entry
                                    .display_names
                                    .insert(display_name_stripped.to_string(), 1);
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
                    if !display_name_stripped.is_empty() {
                        mail_entry
                            .display_names
                            .insert(display_name_stripped.to_string(), 1);
                    }
                    mail_map.insert(email_stripped, mail_entry);
                }
            }
        }
    }
    Ok(mail_map)
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

fn run_queries(
    db: &notmuch::Database,
    query_strings: Vec<String>,
) -> Result<String, notmuch::Error> {
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

                mails = mails.replace(&['\"', '\\', '\t', '\''][..], "");
                collected_mails.push(mails);
            }
        }
    }
    Ok(collected_mails.join(","))
}

fn main() {
    let opt = Opt::from_args();

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
    let map = match retrieve_mail_entries(query_results, &opt.name) {
        Ok(map) => map,
        Err(e) => {
            println!("Error retrieving mails from queries: {}", e);
            return;
        }
    };
    sort_by_count(map);
}
