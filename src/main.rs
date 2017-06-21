extern crate markov;
extern crate tokio_irc_client;
extern crate futures;
extern crate pircolate;
extern crate tokio_core;
extern crate tokio_pool;
extern crate futures_cpupool;
extern crate regex;


use std::collections::HashMap;
use std::{thread, time};

use std::net::ToSocketAddrs;
use tokio_core::reactor::Core;
use futures::{Future, Stream, Sink, stream};
use futures_cpupool::CpuPool;

use tokio_irc_client::Client;
use tokio_irc_client::error::Error as IrcErr;
use tokio_irc_client::error::ErrorKind as IrcErrKind;
use pircolate::message::Message;
use pircolate::message;
use pircolate::message::client::priv_msg;
use pircolate::command::PrivMsg;

pub enum Trig {
    Re(regex::Regex),
    StrPrefix(String),
    Both(regex::Regex, String),
}

trait BotCmd {
    fn process(&self, nick: &str, chan: &str, msg: &str) -> Future<Item=Option<Message>, Error=tokio_irc_client::error::Error>;
    fn trigger() -> Trig;
}


struct CmdHandlers<H>
    where H: BotCmd{
    prefix_map: HashMap<String, Box<H>>,
    re_vec: Vec<(regex::Regex, Box<H>)>,
}

type CmdHandlerss = HashMap<String, Box<BotCmd>>;


fn main() {
    let mut ev = Core::new().unwrap();
    let pool = CpuPool::new_num_cpus();
    let handle = ev.handle();

    // Do a DNS query and get the first socket address for Freenode
    let addr = "localhost:6667".to_socket_addrs().unwrap().next().unwrap();

    let client = Client::new(addr)
        .connect(&handle).and_then(|irc| {
            let connect_sequence = vec! [
                message::client::nick("MarkyBot"),
                message::client::user("MarkyBot", "Markov chain bot"),
                message::client::join("#toplox", None)
            ];

            irc.send_all(stream::iter(connect_sequence))
        }).and_then(|(irc, _)| {
            Ok(irc.split())
        });

    let (sink, stream) = ev.run(client).expect("Failed to connect");
    let sendable_messages = stream.filter_map(|incoming_message| {
        if let Some(pmsg) = incoming_message.command::<PrivMsg>() {
            if let Some((nick, _, _)) = incoming_message.prefix() {
                let pircolate::command::PrivMsg(chan, msg) = pmsg;
                return Some(priv_msg(chan, msg).unwrap());
            }
        }
        return None;
    });
    let procesed_messages = sendable_messages.and_then(|msg| {
        pool.spawn_fn(|| {
            thread::sleep(time::Duration::from_secs(2));
            return Ok(msg);
        })

    });
    let f = sink.send_all(procesed_messages).and_then(|_| {
        Ok(())
    });
    ev.run(f).unwrap();



}
