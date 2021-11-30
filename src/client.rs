extern crate thrussh;
extern crate thrussh_keys;
use crate::error::AsyncSsh2Error;
use std::fmt;
use std::io::Write;
use std::net::IpAddr;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Host {
    Hostname(String),
    IpAddress(IpAddr),
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Host::Hostname(host) => host.to_string(),
                Host::IpAddress(ip) => ip.to_string(),
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AuthMethod {
    Password(String),
}

pub struct Client {
    host: Host,
    port: usize,
    username: String,
    auth: AuthMethod,
    config: Arc<thrussh::client::Config>,
    channel: Option<thrussh::client::Channel>,
}

impl Client {
    pub fn new(host: Host, port: usize, username: String, auth: AuthMethod) -> Self {
        let config = thrussh::client::Config::default();
        let config = Arc::new(config);
        Self {
            host,
            port,
            username,
            auth,
            config,
            channel: None,
        }
    }

    pub async fn connect(&mut self) -> Result<(), AsyncSsh2Error> {
        let handler = Handler::new();
        let config = self.config.clone();
        let addr = self.host.to_string() + ":" + &self.port.to_string();
        let username = self.username.clone();
        let auth = self.auth.clone();
        let mut handle = thrussh::client::connect(config, addr, handler).await?;
        let AuthMethod::Password(password) = auth;
        if handle.authenticate_password(username, password).await? {
            self.channel = Some(handle.channel_open_session().await?);
            Ok(())
        } else {
            Err(AsyncSsh2Error::PasswordWrong)
        }
    }

    pub async fn execute(
        &mut self,
        command: &str,
    ) -> Result<CommandExecutedResult, AsyncSsh2Error> {
        let mut command_execute_result_byte = vec![];
        if let Some(channel) = self.channel.as_mut() {
            channel.exec(true, command).await?;
            while let Some(msg) = channel.wait().await {
                match msg {
                    thrussh::ChannelMsg::Data { ref data } => {
                        command_execute_result_byte.write_all(data).unwrap()
                    }
                    thrussh::ChannelMsg::ExitStatus { exit_status } => {
                        let result = CommandExecutedResult::new(
                            String::from_utf8_lossy(&command_execute_result_byte).to_string(),
                            exit_status,
                        );
                        return Ok(result);
                    }
                    _ => {}
                }
            }
        }

        Err(AsyncSsh2Error::PasswordWrong)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandExecutedResult {
    pub output: String,
    pub exit_status: u32,
}

impl CommandExecutedResult {
    fn new(output: String, exit_status: u32) -> Self {
        Self {
            output,
            exit_status,
        }
    }
}

#[derive(Clone)]
struct Handler;

impl Handler {
    fn new() -> Self {
        Self {}
    }
}

impl thrussh::client::Handler for Handler {
    type Error = AsyncSsh2Error;
    type FutureUnit = std::future::Ready<Result<(Self, thrussh::client::Session), Self::Error>>;
    type FutureBool = std::future::Ready<Result<(Self, bool), Self::Error>>;

    fn finished_bool(self, b: bool) -> Self::FutureBool {
        std::future::ready(Ok((self, b)))
    }
    fn finished(self, session: thrussh::client::Session) -> Self::FutureUnit {
        std::future::ready(Ok((self, session)))
    }
    fn check_server_key(
        self,
        _server_public_key: &thrussh_keys::key::PublicKey,
    ) -> Self::FutureBool {
        self.finished_bool(true)
    }
}

