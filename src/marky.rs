use glob::{glob, GlobError};

use nom;
use nom::IResult::Done;

use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::result::Result;
use std::str::from_utf8;
use std::collections::{HashSet,HashMap, BTreeMap};
use std::path::Path;
use std::cmp;
use std::string::String;

use markov::Chain;

use itertools::Itertools;
use lazysort::SortedBy;
use edit_distance::edit_distance;
use rayon::prelude::*;


named!(irssi_timestr<&str>,
        map_res!(take_until!(" "), from_utf8)
);

named!(irssi_nick<&str>, ws!(
       map_res!(delimited!(tag!("< "), take_until!(">"), char!('>')), from_utf8)
));

named!(znc_timestr<&str>, ws!(
        map_res!(delimited!(char!('['), is_not!("]"), char!(']')), from_utf8)
        )
);
named!(znc_nick<&str>, ws!(map_res!(delimited!(char!('<'), is_not!(">"), char!('>')),from_utf8)));


named!(get_head<(&str, &str)>,
alt_complete!(
  pair!(
       znc_timestr,
       znc_nick
   )|
   pair!(
       irssi_timestr,
       irssi_nick
   )
));

named!(get_msg<((&str, &str), &str)>,
    pair!(
        get_head,
        map_res!(nom::rest, from_utf8)
    )
);

pub fn parse_znc_line(line: &str) -> nom::IResult<&[u8], ((&str, &str), &str)> {
    get_msg(line.as_bytes())
}

fn filter_word(word: &&str) -> bool {
    if word.len() == 0 ||
        word.to_lowercase().starts_with("http://") ||
        word.to_lowercase().starts_with("https://") ||
        !word.chars().any(|c| c.is_alphanumeric()) {
            false
        } else {
            true
        }
}

struct Anal {
    pub user_filter: HashSet<String>,
    pub users: HashMap<String, u64>,
    pub length: HashMap<usize, u64>,
    pub words: HashMap<String, u64>,
    pub lines: HashSet<String>,
    pub chains: HashMap<String, Chain<String>>,
    pub chain: Chain<String>,
}

impl Anal {
    fn update(&mut self, line: ((&str, &str), &str)) {
        let ((_, nick), msg) = line;
        if self.user_filter.contains(nick) {
            return;
        }
        *self.users.entry(nick.to_string()).or_insert(0) += 1;
        *self.length.entry(msg.len()).or_insert(0) += 1;
        let word_iter = msg.split(|c: char| c.is_whitespace())
                    .filter(filter_word);

        for w in  word_iter {
            *self.words.entry(w.into()).or_insert(0) += 1;
        }
        self.lines.insert(msg.into());
        self.chain.feed_str(msg);
    }

    fn print(&mut self) {
        println!("Lengths");
        println!("\t length \t occurence");
        let sorted_l_iter = SortedBy::sorted_by(self.length.iter(),
                                                |&(_, oa), &(_, ob)| reverse(oa.cmp(ob)));
        for (l, occurence) in  sorted_l_iter.take(10) {
            println!("\t {} \t {}", l, occurence);
        }
        println!("Nicks");
        println!("\t nick \t occurence");
        let sorted_nick_iter = SortedBy::sorted_by(self.users.iter(),
                                                  |&(_, oa), &(_, ob)| reverse(oa.cmp(ob)));
        for (nick, occurence) in sorted_nick_iter.take(10) {
            println!("\t {} \t {}", occurence, nick);
        }

        {
            let s = "sie";
            println!("{} -> {:?}", s, self.words.entry(s.into()));
            let s = "Sie";
            println!("{} -> {:?}", s, self.words.entry(s.into()));
            let s = "SIE";
            println!("{} -> {:?}", s, self.words.entry(s.into()));

        }
        let sorted_words = SortedBy::sorted_by(self.words.iter(),
                                              |&(_, oa), &(_, ob)| reverse(oa.cmp(ob)));
        println!("Words");
        println!("\t word \t occurence");
        for (word, occurence) in sorted_words.take(10) {
            println!("\t {} \t {}", occurence, word);
        }

        let common_w_by_popularity = BTreeMap::new();
        let common_w_by_popularity: BTreeMap<usize, (String, u64)> = self.words.iter()
            .fold(common_w_by_popularity, |mut cmap, (word, pop)| {
                if *pop < 10 {
                    return cmap;
                }
                let l = word.len();
                {
                    let wp = cmap.entry(l).or_insert((word.clone(), *pop));
                    if wp.1 < *pop {
                        *wp = (word.clone(), *pop);
                    }
                }
                cmap
            });
        println!("Words by length");
        println!("\t length \t word \t occurence");
        for (word_l, &(ref word, ref pop)) in &common_w_by_popularity {
            println!("\t {} \t {} \t {}", word_l, word, pop);

        }
        let user_vec = self.users.keys()
                .map(|u| u.to_lowercase())
                .collect::<Vec<String>>();
        self.chains = self.lines.iter()
                                .fold(HashMap::new(), |mut chains, line| {
                                    {
                                        let metnioned = user_vec.iter()
                                            .filter(|user| {
                                                line.split(|c: char| c.is_whitespace())
                                                    .map(|w| w.to_lowercase())
                                                    .any(|w| similar_to_nick(&w, user))
                                            });
                                        for mu in metnioned {
                                            chains.entry(mu.clone()).or_insert(Chain::new()).feed_str(line);
                                        }
                                    }
                                    chains
                                });
        let mut existing = 0;
        for (user, chain) in self.chains.iter() {
            let g = chain.generate_str();
            println!("<cyka> {}: {}", user, g);
            if self.lines.contains(&g) {
                existing += 1;
            }
        }
        println!("{} out of {} are not new", existing, self.chains.len());
        println!("l of chains: {}", self.chains.len());
        println!("cyka - {}", self.chain.generate_str());



    }
}

pub fn analyze() {
    let path = "./znc-logs/*.log";
    let mut ignored_nicks: HashSet<_> = HashSet::new();
    ignored_nicks.insert("zn".into());
    let a = Anal{user_filter: ignored_nicks,
                    users: HashMap::new(),
                    length: HashMap::new(),
                    words: HashMap::new(),
                    lines: HashSet::new(),
                    chains: HashMap::new(),
                    chain: Chain::new(),
    };
    let r: Result<Anal, GlobError> = glob(path).expect("glob pattern invalid")
                                        .fold_results(a, analyze_file);
    match r {
        Ok(mut res) => res.print(),
        Err(_) => (),
    }
}

////fn parse_lines<'a, R: Read,>( buf :&'a mut BufReader<R>, ) -> impl Iterator<Item = nom::IResult<&'a [u8], ((&'a str, &'a str), &'a str)>>  {
//fn parse_lines<'a, R: Read>( buf :&'a mut BufReader<R> ) -> impl Iterator<Item =nom::IResult<&'a [u8], ((&'a str, &'a str), &'a str)>>{
//
//
//    buf.lines()
//        .filter_map(Result::ok)
//        .map(|l| get_msg(l.as_bytes()))
//}

fn analyze_file<P: AsRef<Path>>(a: Anal,p: P) -> Anal {
    if let Ok(f) = File::open(p) {
       let file = BufReader::new(&f);
       file.lines()
           .filter_map(Result::ok)
           .fold(a, |mut a, l| {
               if let Done(_, line) = parse_znc_line(&l) {
                   a.update(line);
               }
               a
           })
    } else {
        a

    }

}

fn similar_to_nick(word: &str, nick: &str) -> bool {

    if word == nick {
        true
    } else {
        if word.len() < 4  || nick.len() < 4 {
            return false;
        }
        let q = nick.len() / 3;

        // dirty hax
        edit_distance(&nick, word) as usize <= q
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn znc_line_test() {
        let test_str = "[00:47:47] <emilsp> Dawgora: kekekekekekekekekekekekek";
        let result = get_msg(test_str.as_bytes());
        assert!(!result.is_err());
        println!("result -> {:?}", result);
    }

    #[test]
    fn irssi_line_test() {
        let test_str = "07:37 < Aleksejs> http://www.sciencealert.com/images/articles/processed/PurityXKCD_web_1024.jpg";
        let result = get_msg(test_str.as_bytes());
        println!("r -> {:?}", result);
        assert!(!result.is_err());
    }

    #[test]
    fn test_similarity() {
        assert!(!similar_to_nick("pisies", "sie"));
        assert!(similar_to_nick("alolsejs", "aleksejs"));
        assert!(similar_to_nick("aloxsej", "aleksejs"));
        assert!(!similar_to_nick("panika", "tatra"));
        assert!(!similar_to_nick("dianshi,", "tatra"));
    }
}

fn reverse(o: cmp::Ordering) -> cmp::Ordering {
    use std::cmp::Ordering as O;
    match o {
        O::Greater => O::Less,
        O::Less => O::Greater,
        O::Equal => O::Equal,
    }
}
