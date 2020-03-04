use std::fs::File;
use std::io::Read;

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


// query:
// collect all messages, where to: search and from is me
// count
// if too small, collet all from: search and to egal
// go through all headers, split on commas, match occurences and sort them descending

fn main() {
    let opt = Opt::from_args();

    // Load the user's config
    let mut home = match dirs::home_dir() {
        Some(dir) => dir,
        None => {
            println!("Could not find configuration file neomutt_address.toml");
            return;
        }
    };
    home.push(".vimrc");

    println!("{:?}", home);

    // let config = match xdg.find_config_file("buzz.toml") {
    //     Some(config) => config,
    //     None => {
    //         println!("Could not find configuration file buzz.toml");
    //         return;
    //     }
    // };
    // let config = {
    //     let mut f = match File::open(config) {
    //         Ok(f) => f,
    //         Err(e) => {
    //             println!("Could not open configuration file buzz.toml: {}", e);
    //             return;
    //         }
    //     };
    //     let mut s = String::new();
    //     if let Err(e) = f.read_to_string(&mut s) {
    //         println!("Could not read configuration file buzz.toml: {}", e);
    //         return;
    //     }
    //     match s.parse::<toml::Value>() {
    //         Ok(t) => t,
    //         Err(e) => {
    //             println!("Could not parse configuration file buzz.toml: {}", e);
    //             return;
    //         }
    //     }
    // };

    // let name = "";
    // let mails = vec![];

    // let mut mail_path = dirs::home_dir().unwrap(); mail_path.push(".mail/uni");

    // let blubb : Vec<_> = mails.iter().map(|mail| { format!("from: {}", mail) }).collect();
    // let mut query_string = blubb.join(" or ");
    // query_string = format!("({}) and to: {}", query_string, name);
    // println!("{}", query_string);



    // let db = notmuch::Database::open(&mail_path, notmuch::DatabaseMode::ReadOnly).unwrap();
    // let query = db.create_query(&query_string).unwrap();
    // let count = query.count_messages();
    // println!("{:?}", count);

    // let threads = query.search_messages().unwrap();
    // for thread in threads {
    //     println!("thread {:?} ", thread.header("to"));
    // }


}
