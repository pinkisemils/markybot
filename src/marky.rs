use glob::{glob, GlobError};
use nom;
use nom::IResult::Done;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::result::Result;
use std::str::from_utf8;
use std::collections::HashMap;
use itertools::Itertools;
use std::path::Path;

named!(timestr<&str>, ws!(
        map_res!(delimited!(char!('['), is_not!("]"), char!(']')), from_utf8)
        )
    );
named!(nick<&str>, ws!(map_res!(delimited!(char!('<'), is_not!(">"), char!('>')),from_utf8)));


named!(get_head<(&str, &str)>,
pair!(
    timestr,
    nick
 ));

named!(get_msg<((&str, &str), &str)>,
    pair!(
        get_head,
        map_res!(nom::rest, from_utf8)
    )
);

pub fn parse_line(line: &str) -> nom::IResult<&[u8], ((&str, &str), &str)> {
    get_msg(line.as_bytes())
}

struct Anal {
    pub users: HashMap<String, u64>,
    pub length: HashMap<usize, u64>,
}

impl Anal {
    fn update(&mut self, line: ((&str, &str), &str)) {
        let ((_, nick), msg) = line;
        *self.users.entry(nick.to_string()).or_insert(0) += 1;
        *self.length.entry(msg.len()).or_insert(0) += 1;
    }

    fn print(&self) {
        println!("Num of unique lengths: {}", self.users.len());

    }
}

pub fn analyze() {
    let path = "./znc-logs/*.log";
    let a = Anal{users: HashMap::new(), length: HashMap::new()};
    let r: Result<Anal, GlobError> = glob(path).expect("glob pattern invalid")
                                        .fold_results(a, analyze_file);
    match r {
        Ok(res) => res.print(),
        Err(_) => (),
    }
}

fn analyze_file<P: AsRef<Path>>(a: Anal,p: P) -> Anal {
    if let Ok(f) = File::open(p) {
       let file = BufReader::new(&f);
       file.lines()
           .filter_map(Result::ok)
           .fold(a, |mut a, l| {
               if let Done(_, line) = parse_line(&l) {
                   a.update(line);
               }
               a
           })
    } else {
        a

    }

}


#[cfg(test)]
mod test {
    use super::*;

    fn line_test() {
        let test_str = "[00:47:47] <emilsp> Dawgora: kekekekekekekekekekekekek";
        let result = get_msg(test_str.as_bytes());
        println!("result -> {:?}", result);
    }
}
