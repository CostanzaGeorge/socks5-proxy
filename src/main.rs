#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_must_use)]
#![allow(unused_variables)]

extern crate byteorder;
extern crate rand;
extern crate requests;

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::str;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use rand::Rng;
use requests::{get};

/// Read the first TCP packet in the Socks5 Client<-->Proxy Server transaction.
/// This packet is called the "Client-Greeting" packet. It contains the Socks version
/// and authentication information.
///
fn read_client_greeting(stream: &mut TcpStream) {

    // Read the Socks version byte.
    if stream.read_u8().unwrap() != 5 {
        println!("[-] Received invalid socks version.");
    } else {
        println!("[+] Received valid Socks version.");
    }

    // Read the "number of auth methods" byte.
    let n_auth_methods = stream.read_u8().unwrap();

    // Read the available auth methods.
    let mut auth_methods: Vec<u8> = vec![];
    for _ in 0..n_auth_methods {
        auth_methods.push(stream.read_u8().unwrap());
    }

    println!("log: Client-Greeting read successfully.");
}

/// Send the 2nd TCP packet from the Proxy Server to the Client. This packet verifies
/// which version of Socks that the Server is using and its authentication mechanism.
///
fn send_auth_choice(stream: &mut TcpStream) {
    stream.write_all(&vec![5, 0]);
    println!("log: Server's auth-choice sent successfully.")
}

/// Read the 3rd packet in the Socks5 transaction. The "Client-Connection-Request" packet
/// is used by the Client to tell the Proxy what the address is of the Destination Server
/// and how the Proxy should connect to it (the "command-code").
///
/// # Returns:
///
/// `dest_addr`: String - The IPv4 address of the Destination Server.
///
fn read_client_conn_req(stream: &mut TcpStream) -> String {

    // Read the Socks version byte, this must be 5.
    if stream.read_u8().unwrap() != 5 {
        println!("[-] Received invalid socks version.");
    }

    // Read the Client's command-code, this must be 1.
    if stream.read_u8().unwrap() != 1 {
        println!("[-] Received invalid command-code.");
    }

    // The "reserved-byte" must be zero.
    if stream.read_u8().unwrap() != 0 {
        println!("[-] Received invalid reserved byte.");
    }

    // Read the destination address-type, this must be an IPv4 address.
    if stream.read_u8().unwrap() != 1 {
        println!("[-] Received invalid destination address-type.");
    }

    // Read the destination address (an IPv4 address followed by a port number).
    let dest_addr: String = format!(
        "{}.{}.{}.{}:{}",
        stream.read_u8().unwrap(),
        stream.read_u8().unwrap(),
        stream.read_u8().unwrap(),
        stream.read_u8().unwrap(),
        stream.read_u16::<BigEndian>().unwrap(),
    );

    println!("log: Client-Connection-Request read successfully.");
    dest_addr
}

/// Send the Client the the Proxy's response to the "Connection-Requeset" packet.
///
/// TODO:
///     - Does the client need to know the IP and Port here? For now I am just sending zeros.
///     - Also, cURL dosen't complain about this being blank, so idk.
///
fn send_conn_response(stream: &mut TcpStream) {
    stream.write_all(&vec![5, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
    println!("[+] Successfully sent the Server's Connection-Response.");
}

/// Read a GET request from the TcpStream. This reads from the stream until it reaches the
/// HTTP double-carriage: \r\n\r\n (used to seperate a Request's headers from its body).
///
/// This function assumes that the Socks5 Client's GET request has no data. This should be changed.
///
/// # Returns:
///
/// `header_buf`: Vec<u8> - The bytes from the Client's GET requests (minus any body, if one was sent).
///
fn read_client_get_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut header_buf: Vec<u8> = vec![];
    let mut carriage_count = 0;
    let mut done = false;

    // Read the byte-stream until you reach 4 consecutive bytes: b"\r\n\r\n".
    while !done {
        let b = stream.read_u8().unwrap();

        if b == b'\r' {
            if carriage_count == 0 {
                carriage_count += 1;
            } else if header_buf[header_buf.len() - 1] == b'\n' {
                carriage_count += 1;
            } else {
                carriage_count = 0;
            }
        } else if b == b'\n' {
            if header_buf[header_buf.len() - 1] == b'\r' {
                carriage_count += 1;
            } else {
                carriage_count = 0;
            }
        } else {
            carriage_count = 0;
        }

        header_buf.push(b);
        if carriage_count == 4 { done = true; }
    }

    header_buf
}

fn main() {
    println!("log: The Pronoun-Proxy is listening on: 0.0.0.0:9093...");
    let pronouns = vec![
        "he", "him", "his", "himself",
        "she", "her", "hers", "herself"
	];

    let listener = TcpListener::bind("0.0.0.0:9093").unwrap();

    for client_stream in listener.incoming() {
        match client_stream {
            Ok(mut client_stream) => {

                println!("log: Accepted client connection.");
                read_client_greeting(&mut client_stream);
                send_auth_choice(&mut client_stream);

                let dest_addr: String = read_client_conn_req(&mut client_stream);
                let mut dest_stream = TcpStream::connect(dest_addr).unwrap();
                send_conn_response(&mut client_stream);
                let get_req_buf: Vec<u8> = read_client_get_request(&mut client_stream);
                println!("log: Receive HTTP GET Request from Client.");

                println!("log: Forwarding GET Request to Destination.");
                dest_stream.write_all(&get_req_buf);

                let mut in_buf: Vec<u8> = vec![];
                let mut out_buf: Vec<u8> = vec![];
                dest_stream.read_to_end(&mut in_buf);
                println!("log: Received GET Response from Destination.");

                let buf_reader = BufReader::new(&in_buf[..]);
                // A marker for controlling what processing function is run when iterating over the lines in GET Response.
                let mut reached_body = false;
                // The index in the output buffer for where to insert the new "Content-Length" header value.
                let mut cl_index = 0;
                // The length in bytes of the output buffer's HTTP headers.
                let mut header_len = 0;

                // For each line in the GET request:
                //
                // (1) If the line is a header, and not the "Content-Length" header, write the
                //     bytes to the output buffer.
                // (2) If the line is the "Content-Length" header, mark the index in the output
                //     buffer where you will have to insert an updated body byte-length (this is
                //     because we are changing the packet's original body, so the Content-Length
                //     will change with time.
                // (3) If the line is part of the packet's body, replace each pronoun with a
                //     new pronoun.
                //
                for line in buf_reader.lines() {
                    let line = line.unwrap();

                    if line == "" {
                        // The previous header's bytes will supply the first b"\r\n" in the HTTP
                        // packet's double-carriage return.
                        out_buf.extend(b"\r\n");
                        reached_body = true;
                        header_len = out_buf.len();
                        continue;
                    }

                    // Randomly shuffle the gendered pronouns in the packet's body.
                    if reached_body {
                        let words: Vec<&str> = line.split(" ")
                            .map(|word| {
                                if pronouns.contains(&word) {
                                    rand::thread_rng().choose(&pronouns).unwrap()
                                } else {
                                    word
                                }
                            })
                            .collect();
                        let replaced = words.join(" ");
                        out_buf.extend(replaced.as_bytes());
                    } else {
                        // Mark the index in the output-buffer for where to insert the
                        // Content-Length of the new packet's text.
                        if line.starts_with("Content-Length") {
                            out_buf.extend(b"Content-Length ");
                            cl_index = out_buf.len();
                        // For every header other than the "Content-Length" in the input buffer,
                        // just copy the bytes into the output buffer.
                        } else {
                            out_buf.extend(line.as_bytes());
                        }
                        out_buf.extend(b"\r\n");
                    }
                }

                // When iterating via .lines(), the trailing "\n" gets stripped off. Add it back in.
                out_buf.extend(b"\n");

                // Calculate the "Content-Length" header for the new GET Response packet.
                let content_len = out_buf.len() - header_len;
                println!("log: Updated Content-Length: {:?}", content_len);

                // Insert the new "Content-Length" value into the output buffer.
                for &b in format!("{}", content_len).as_bytes() {
                    out_buf.insert(cl_index, b);
                    cl_index += 1;
                }
                println!("log: Finished switching pronouns in Response body.");

                // Send the new GET Response to the Socks Client.
                client_stream.write_all(&out_buf);
                println!("log: Finished - new GET Response successfully sent to Client.");
            },
            Err(why) => println!("Error: {:?}", why)
        }
    }

    println!("log: Closing the Pronoun-Proxy server.");
}
