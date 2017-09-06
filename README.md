
# The `Socks5 Pronoun-Proxy`


## About

This code implements a Socks5 Proxy used to randomly shuffle the pronouns in the body of HTTP GET requests.
This proxy listens on `127.0.0.1:9093`. It listens for TCP connections from a Socks5 Client.
When that TCP connection is received, a Socks5 connection is established between the Socks5 Client and Proxy Server.
Once established, the Client can make HTTP GET requests to some Destination server through the TCP tunnel via the Proxy.
The Proxy will shuffle the gendered pronouns in the GET response received from the Destination server.


## Requires

    - Rust Nightly, at the time of writing this, this is: `1.20.0-nightly`.


## Install (if not using Docker)

```
$ git clone https://github.com/CostanzaGeorge/code-samples.git
$ cd pronoun-proxy
$ cargo run
```

## Testing

In terminal #1 - Serve a static file (here, on port 8000) containing text with pronouns.

```
$ mkdir testing && cd testing
$ echo "she talked with him" > file.txt
$ python3.6 -m http.server
```

In terminal #2 - Run the `pronoun-proxy` (here, on port 9093).

```
$ cd $LOCATION_OF_PRONOUN_PROXY
$ cargo run
```

In terminal #3 - cURL the Static Server via the Proxy.

```
$ curl --proxy socks5://127.0.0.1:9093 http://127.0.0.1:8000/file.txt > test.txt
$ cat test.txt
# => $RANDOM_PRONOUN talked with $RANDOM_PRONOUN
```
