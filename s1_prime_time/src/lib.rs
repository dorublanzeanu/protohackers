use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use server::{Server, ServerErrorKind};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufStream};
use tokio::net::{TcpListener, TcpStream};

/// Prime Time
///
/// This Service receives JSON Formatted objects that specify a number to
/// be checked if is prime
///
/// A conforming request object has the required field method,
/// which must always contain the string `isPrime`, and the required field number,
/// which must contain a number.
/// Any JSON number is a valid number, including floating-point values.
///
/// Input Example:
/// `{"method":"isPrime","number":123}`
///
/// A request is malformed if it is not a well-formed JSON object,
/// if any required field is missing, if the method name is not `isPrime`,
/// or if the number value is not a number.
///
/// Extraneous fields are to be ignored.
///
/// A conforming response object has the required field method,
/// which must always contain the string `isPrime`, and the required field prime,
/// which must contain a boolean value: true if the number in the request was prime,
/// false if it was not.
///
/// Output Example:
///  `{"method":"isPrime","prime":false}`
///
/// A response is malformed if it is not a well-formed JSON object, if any required field is missing,
/// if the method name is not `isPrime`, or if the prime value is not a boolean.
///
/// Accept TCP connections.
///
/// Whenever you receive a conforming request, send back a correct response, and wait for another request.
///
/// Whenever you receive a malformed request, send back a single malformed response, and disconnect the client.
///
/// Make sure you can handle at least 5 simultaneous clients.
#[derive(Debug, Default)]
pub struct PrimeTimeServer;

#[async_trait]
impl Server for PrimeTimeServer {
    /// Method that starts the server
    async fn run(&mut self, addr: &str) -> Result<(), ServerErrorKind> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|_| ServerErrorKind::BindFail)?;

        println!("Listening on {:?}", addr);

        loop {
            println!("Waiting for connection ...");

            // The second item contains the IP and port of the new connection.
            let (socket, _) = listener.accept().await.unwrap();

            println!("Connection open\n");

            // A new task is spawned for each inbound socket. The socket is
            // moved to the new task and processed there.
            tokio::spawn(async move { process(socket).await });
        }
    }
}

/// Processes a connection
///
/// Returns a `Result` which is empty on the success path and
/// contains a `ServerErrorKind` on the error path
async fn process(stream: TcpStream) -> Result<(), ServerErrorKind> {
    let mut stream = BufStream::new(stream);
    let mut line = vec![];
    let mut should_continue = true;

    while should_continue {
        let read_len = stream
            .read_until(b'\n', &mut line)
            .await
            .map_err(|_| ServerErrorKind::ReadFail)?;

        if read_len > 0 {
            // Construct a request from the u8 vec
            let req = Request::from_bytes(line.as_slice());

            // Consume the request and construct a response
            let resp = req.process();

            // return an error if the response is malformed
            // otherwise return the response
            let response = match resp {
                Response::ConformingResp { .. } => resp.into_bytes(),
                Response::MalformedResp => {
                    should_continue = false;
                    resp.into_bytes()
                }
            };

            // If there's something to send
            if !response.is_empty() {
                // Send back the result
                stream
                    .write_all(&response)
                    .await
                    .map_err(|_| ServerErrorKind::WriteFail)?;

                // Flush the buffer to ensure it is sent
                stream
                    .flush()
                    .await
                    .map_err(|_| ServerErrorKind::WriteFail)?;
            }
        } else {
            should_continue = false;
        }

        line.clear();
    }

    Ok(())
}

/// Conforming Request object
/// Used for deserializing JSON bytes received
#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct ConformingReqObj {
    pub method: String,
    pub number: f64,
}

/// Conforming Response object
/// Used for serializing JSON before sending
#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct ConformingRespObj {
    method: String,
    prime: bool,
}

/// Request type
/// Constructed based on bytes and verified if it
/// satisfies the solution conditions
#[derive(Debug, PartialEq)]
enum Request {
    ConformingReq { method: String, number: f64 },
    MalformedReq,
}

/// Response type
/// Constructed based on a processed request
#[derive(Debug, PartialEq)]
enum Response {
    ConformingResp { method: String, prime: bool },
    MalformedResp,
}

impl Request {
    /// Creates a `Request` from provided bytes
    /// and verifies if it satisfies the solution conditions
    ///
    /// If it does, it returns a `Request::ConformingReq`
    /// Otherwise, it returns a `Request::MalformedReq`
    fn from_bytes(line: &[u8]) -> Request {
        let obj = serde_json::from_slice::<ConformingReqObj>(line).ok();

        match obj {
            Some(ConformingReqObj { method, number }) => {
                if method == "isPrime" {
                    Request::ConformingReq { method, number }
                } else {
                    Request::MalformedReq
                }
            }
            None => Request::MalformedReq,
        }
    }

    /// Processes the request and returns a `Response`
    fn process(self) -> Response {
        match self {
            Request::ConformingReq { method, number } => Response::ConformingResp {
                method,
                prime: is_prime(number),
            },
            Request::MalformedReq => Response::MalformedResp,
        }
    }
}

impl Response {
    /// Converts the response into bytes ready to be sent
    fn into_bytes(self) -> Vec<u8> {
        match self {
            Response::ConformingResp { method, prime } => {
                let obj = ConformingRespObj { method, prime };
                let mut res = serde_json::to_string(&obj).unwrap();

                // Add newline
                res.push('\n');
                res.as_bytes().to_vec()
            }
            Response::MalformedResp => b"malformed\n".to_vec(),
        }
    }
}

/// Checks if a number is prime
fn is_prime(number: f64) -> bool {
    let n = number;
    let number = number as i64;

    if n.fract() != 0.0 || number < 2 {
        false
    } else {
        let end = f64::sqrt(number as f64) as i64;

        !(2..=end).any(|n| number % n == 0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_deserialize_req() {
        let line = b"{\"method\":\"isPrime\",\"number\":123}";

        let obj: ConformingReqObj = serde_json::from_slice(line).unwrap();
        assert_eq!(obj.method, "isPrime");
        assert_eq!(obj.number, 123f64);
    }

    #[test]
    fn test_deserialize_inverted_req() {
        let line = b"{\"number\":2,\"method\":\"isPrime\"}";

        let obj: ConformingReqObj = serde_json::from_slice(line).unwrap();
        assert_eq!(obj.method, "isPrime");
        assert_eq!(obj.number, 2f64);
    }

    #[test]
    fn test_deserialize_inverted_newline_req() {
        let line = b"{\"number\":2,\"method\":\"isPrime\"}\n";

        let obj: ConformingReqObj = serde_json::from_slice(line).unwrap();
        assert_eq!(obj.method, "isPrime");
        assert_eq!(obj.number, 2f64);
    }

    #[test]
    fn test_serialize_deserialize_resp() {
        let obj_0 = ConformingRespObj {
            method: "isPrime".to_string(),
            prime: true,
        };

        let ser = serde_json::to_string(&obj_0).unwrap().as_bytes().to_vec();
        let obj_1: ConformingRespObj = serde_json::from_slice(&ser).unwrap();

        assert_eq!(obj_0, obj_1);
    }

    #[test]
    fn test_req_valid_success() {
        let line = b"{\"method\":\"isPrime\",\"number\":123}";
        let req = Request::from_bytes(line);

        assert_eq!(
            req,
            Request::ConformingReq {
                method: "isPrime".to_string(),
                number: 123f64
            }
        );
    }

    #[test]
    fn test_req_valid_inverted_success() {
        let line = b"{\"number\":2,\"method\":\"isPrime\"}";
        let req = Request::from_bytes(line);

        assert_eq!(
            req,
            Request::ConformingReq {
                method: "isPrime".to_string(),
                number: 2f64
            }
        );
    }

    #[test]
    fn test_req_malformed_json_error() {
        let line = b"\"method\":\"isPrim\",\"number\":123}";
        let req = Request::from_bytes(line);

        assert_eq!(req, Request::MalformedReq);
    }

    #[test]
    fn test_req_malformed_invalid_method_error() {
        let line = b"{\"method\":\"isPrim\",\"number\":123}";
        let req = Request::from_bytes(line);

        assert_eq!(req, Request::MalformedReq);
    }

    #[test]
    fn test_req_malformed_number_error() {
        let line = b"{\"method\":\"isPrime\",\"number\":\"123\"}";
        let req = Request::from_bytes(line);

        assert_eq!(req, Request::MalformedReq);
    }

    #[test]
    fn test_req_process_prime_success() {
        let line = b"{\"method\":\"isPrime\",\"number\":11}";
        let req = Request::from_bytes(line);

        assert_eq!(
            req,
            Request::ConformingReq {
                method: "isPrime".to_string(),
                number: 11f64
            }
        );
        assert_eq!(
            req.process(),
            Response::ConformingResp {
                method: "isPrime".to_string(),
                prime: true
            }
        );
    }

    #[test]
    fn test_req_process_prime_inverted_success() {
        let line = b"{\"number\":2,\"method\":\"isPrime\"}";
        let req = Request::from_bytes(line);

        assert_eq!(
            req,
            Request::ConformingReq {
                method: "isPrime".to_string(),
                number: 2f64
            }
        );
        assert_eq!(
            req.process(),
            Response::ConformingResp {
                method: "isPrime".to_string(),
                prime: true
            }
        );
    }

    #[test]
    fn test_req_process_not_prime_success() {
        let line = b"{\"method\":\"isPrime\",\"number\":9}";
        let req = Request::from_bytes(line);

        assert_eq!(
            req,
            Request::ConformingReq {
                method: "isPrime".to_string(),
                number: 9f64
            }
        );
        assert_eq!(
            req.process(),
            Response::ConformingResp {
                method: "isPrime".to_string(),
                prime: false
            }
        );
    }

    #[test]
    fn test_req_process_prime_2_success() {
        let line = b"{\"method\":\"isPrime\",\"number\":2}";
        let req = Request::from_bytes(line);

        assert_eq!(
            req,
            Request::ConformingReq {
                method: "isPrime".to_string(),
                number: 2f64
            }
        );
        assert_eq!(
            req.process(),
            Response::ConformingResp {
                method: "isPrime".to_string(),
                prime: true
            }
        );
    }

    #[test]
    fn test_req_to_vec_prime_success() {
        let line = b"{\"method\":\"isPrime\",\"number\":778013}\n";
        let req = Request::from_bytes(line);

        assert_eq!(
            req,
            Request::ConformingReq {
                method: "isPrime".to_string(),
                number: 778013f64
            }
        );
        let resp = req.process();
        assert_eq!(
            resp,
            Response::ConformingResp {
                method: "isPrime".to_string(),
                prime: true
            }
        );
        assert_eq!(
            resp.into_bytes(),
            "{\"method\":\"isPrime\",\"prime\":true}\n"
                .to_string()
                .as_bytes()
                .to_vec()
        );
    }

    #[test]
    fn test_req_to_vec_prime_inverted_success() {
        let line = b"{\"number\":2,\"method\":\"isPrime\"}";
        let req = Request::from_bytes(line);

        assert_eq!(
            req,
            Request::ConformingReq {
                method: "isPrime".to_string(),
                number: 2f64
            }
        );
        let resp = req.process();
        assert_eq!(
            resp,
            Response::ConformingResp {
                method: "isPrime".to_string(),
                prime: true
            }
        );
        assert_eq!(
            resp.into_bytes(),
            "{\"method\":\"isPrime\",\"prime\":true}\n"
                .to_string()
                .as_bytes()
                .to_vec()
        );
    }

    #[test]
    fn test_req_to_vec_prime_0_success() {
        let line = b"{\"number\":0,\"method\":\"isPrime\"}";
        let req = Request::from_bytes(line);

        assert_eq!(
            req,
            Request::ConformingReq {
                method: "isPrime".to_string(),
                number: 0f64
            }
        );
        let resp = req.process();
        assert_eq!(
            resp,
            Response::ConformingResp {
                method: "isPrime".to_string(),
                prime: false
            }
        );
        assert_eq!(
            resp.into_bytes(),
            "{\"method\":\"isPrime\",\"prime\":false}\n"
                .to_string()
                .as_bytes()
                .to_vec()
        );
    }

    #[test]
    fn test_req_to_vec_prime_1_success() {
        let line = b"{\"number\":1,\"method\":\"isPrime\"}";
        let req = Request::from_bytes(line);

        assert_eq!(
            req,
            Request::ConformingReq {
                method: "isPrime".to_string(),
                number: 1f64
            }
        );
        let resp = req.process();
        assert_eq!(
            resp,
            Response::ConformingResp {
                method: "isPrime".to_string(),
                prime: false
            }
        );
        assert_eq!(
            resp.into_bytes(),
            "{\"method\":\"isPrime\",\"prime\":false}\n"
                .to_string()
                .as_bytes()
                .to_vec()
        );
    }

    #[test]
    fn test_req_to_vec_not_prime_negative_success() {
        let line = b"{\"number\":-3,\"method\":\"isPrime\"}";
        let req = Request::from_bytes(line);

        assert_eq!(
            req,
            Request::ConformingReq {
                method: "isPrime".to_string(),
                number: -3f64
            }
        );
        let resp = req.process();
        assert_eq!(
            resp,
            Response::ConformingResp {
                method: "isPrime".to_string(),
                prime: false
            }
        );
        assert_eq!(
            resp.into_bytes(),
            "{\"method\":\"isPrime\",\"prime\":false}\n"
                .to_string()
                .as_bytes()
                .to_vec()
        );
    }

    #[test]
    fn test_req_to_vec_float_success() {
        let line = b"{\"method\":\"isPrime\",\"number\":4224223.1234}\n";
        let req = Request::from_bytes(line);

        assert_eq!(
            req,
            Request::ConformingReq {
                method: "isPrime".to_string(),
                number: 4224223.1234f64
            }
        );
        let resp = req.process();
        assert_eq!(
            resp,
            Response::ConformingResp {
                method: "isPrime".to_string(),
                prime: false
            }
        );
        assert_eq!(
            resp.into_bytes(),
            "{\"method\":\"isPrime\",\"prime\":false}\n"
                .to_string()
                .as_bytes()
                .to_vec()
        );
    }
}
