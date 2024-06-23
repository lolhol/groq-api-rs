use std::hash::{Hash, Hasher};

use super::{
    message::Message,
    request,
    response::{ErrorResponse, Response},
};
use crate::completion::response::StreamResponse;
use futures::StreamExt;
use reqwest::header;
use reqwest_eventsource::{Event, EventSource};

#[derive(Debug, Clone)]
/// The returned response from groq's completion API could either be a json with full llm response
/// or chunks of response sent via Server Sent Event(SSE)
pub enum CompletionOption {
    NonStream(Response),
    Stream(Vec<StreamResponse>),
}

/// # Private Fields
/// - api_key, the API key used to authenticate with groq,
/// - client, the reqwest::Client with built in connection pool,
/// - tmp_messages, messages that stay there for only a single request. After the request they are cleared.
/// - messages,  a Vec for containing messages send to the groq completion endpoint (historic messages will not clear after request)
#[derive(Debug, Clone)]
pub struct Groq {
    api_key: String,
    messages: Vec<Message>,
    tmp_messages: Vec<Message>,
    client: reqwest::Client,
}

impl Groq {
    pub fn new(api_key: &str) -> Self {
        //! Returns an instance of Groq struct.
        //! ```ignore no_run
        //! Self {
        //!     api_key: api_key.into(), // the API key used to authenticate with groq
        //!     client: reqwest::Client::new(), // the reqwest::Client with built in connection pool
        //!     messages: Vec::new() // a Vec for containing messages send to the groq completion endpoint (historic messages will not clear after request)
        //! }
        //! ```
        Self {
            api_key: api_key.into(),
            client: reqwest::Client::new(),
            tmp_messages: Vec::new(),
            messages: Vec::new(),
        }
    }

    pub fn add_message(mut self, msg: Message) -> Self {
        //! Adds a message to the internal message vector
        self.messages.push(msg);
        self
    }

    pub fn add_messages(mut self, msgs: Vec<Message>) -> Self {
        //! Add messages to the internal message vector
        self.messages.extend(msgs);
        self
    }

    pub fn clear_messages(mut self) -> Self {
        //! Clears the internal message vector.
        //! And shrink the capacity to 3.
        self.messages.clear();
        self.messages.shrink_to(3);
        self
    }

    /// Clears the internal tmp_messages vector.
    /// # Note
    /// Fucntion is created for internal use and is not recomended for external use.
    pub fn clear_tmp_messages_override(&mut self) {
        self.tmp_messages.clear();
    }

    pub fn add_tmp_messages(mut self, msgs: Vec<Message>) -> Self {
        self.tmp_messages.extend(msgs);
        self
    }

    pub fn add_tmp_message(mut self, msg: Message) -> Self {
        self.tmp_messages.push(msg);
        self
    }

    fn get_tmp_request_messages(&self) -> Option<Vec<Message>> {
        if self.tmp_messages.is_empty() {
            None
        } else {
            Some(self.tmp_messages.clone())
        }
    }

    /// Outputs the request messages that should be passed onto the request.
    /// Utility function created for easier logic internally.
    fn get_all_request_messages(&self) -> Vec<Message> {
        if self.tmp_messages.is_empty() {
            self.messages.clone()
        } else {
            return vec![self.tmp_messages.clone(), self.messages.clone()].concat();
        }
    }

    /// Outputs the request messages that should be passed onto the request and clears the tmp messages.
    /// Utility function created for easier logic internally.
    fn get_request_messages_with_tmp_clear(&mut self) -> Vec<Message> {
        let all = self.get_all_request_messages();
        self.clear_tmp_messages_override();
        return all;
    }

    async fn create_stream_completion(
        &mut self,
        req: request::builder::RequestBuilder,
    ) -> anyhow::Result<CompletionOption> {
        /* REMARK:
         * https://github.com/jpopesculian/reqwest-eventsource/
         * https://parsec.cloud/en/how-the-reqwest-http-client-streams-responses-in-a-web-context/
         */
        let req = req
            .with_messages(self.get_request_messages_with_tmp_clear())?
            .build();
        anyhow::ensure!(
            req.is_stream(),
            "'create_stream_completion' func must have the stream flag turned on in request body"
        );
        let mut stream = EventSource::new(
            self.client
                .post("https://api.groq.com/openai/v1/chat/completions")
                .header(header::AUTHORIZATION, format!("Bearer {}", self.api_key))
                .header(header::ACCEPT, "text/event-stream")
                .json(&req),
        )?;
        let mut bufs: Vec<StreamResponse> = Vec::new();
        while let Some(event) = stream.next().await {
            match event {
                Ok(Event::Open) => println!("Connection Open!"),
                Ok(Event::Message(message)) => {
                    if message.data == "[DONE]" {
                        break;
                    }
                    bufs.push(serde_json::from_str(&message.data)?);
                }
                Err(err) => {
                    stream.close();
                    anyhow::bail!("Error: {}", err);
                }
            }
        }
        stream.close();

        Ok(CompletionOption::Stream(bufs))
    }

    async fn create_non_stream_completion(
        &mut self,
        req: request::builder::RequestBuilder,
    ) -> anyhow::Result<CompletionOption> {
        let req = req
            .with_messages(self.get_request_messages_with_tmp_clear())?
            .build();
        let body = (self.client)
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header(header::AUTHORIZATION, format!("Bearer {}", self.api_key))
            .json(&req)
            .send()
            .await?;
        if body.status() == reqwest::StatusCode::OK {
            Ok(CompletionOption::NonStream(body.json::<Response>().await?))
        } else {
            let statcode = body.status().clone();
            let mut error: ErrorResponse = serde_json::from_str(&body.text().await?)?;
            error.code = statcode;
            anyhow::bail!(error)
        }
    }

    pub async fn create(
        &mut self,
        req: request::builder::RequestBuilder,
    ) -> anyhow::Result<CompletionOption> {
        if !req.is_stream() {
            self.create_non_stream_completion(req).await
        } else {
            self.create_stream_completion(req).await
        }
    }
}

impl Hash for Groq {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.messages.hash(state);
        self.api_key.hash(state);
    }
}

#[cfg(test)]
mod completion_test {
    use std::hash::{DefaultHasher, Hash, Hasher};

    use crate::completion::{client::Groq, message::Message, request::builder};

    #[test]
    fn test_eq_and_hash() {
        let g1 = Groq::new("api_key").add_messages(vec![Message::UserMessage {
            role: Some("user".to_string()),
            content: Some("Explain the importance of fast language models".to_string()),
            name: None,
            tool_call_id: None,
        }]);

        let g2 = Groq::new("api_key").add_messages(vec![Message::UserMessage {
            role: Some("user".to_string()),
            content: Some("Explain the importance of fast language models".to_string()),
            name: None,
            tool_call_id: None,
        }]);

        let mut hasher = DefaultHasher::new();
        let mut hasher1 = DefaultHasher::new();

        g1.hash(&mut hasher);
        g2.hash(&mut hasher1);
        let hash_string = hasher.finish();
        let hash_string1 = hasher1.finish();

        assert_eq!(hash_string, hash_string1);
    }

    #[tokio::test]
    async fn create_completion() -> anyhow::Result<()> {
        let messages = vec![Message::UserMessage {
            role: Some("user".to_string()),
            content: Some("Explain the importance of fast language models".to_string()),
            name: None,
            tool_call_id: None,
        }];
        let request = builder::RequestBuilder::new("mixtral-8x7b-32768".to_string());
        let api_key = env!("GROQ_API_KEY");

        let client = Groq::new(api_key);
        let mut client = client.add_messages(messages);

        let res = client.create(request).await;
        assert!(res.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn create_stream_completion() -> anyhow::Result<()> {
        let messages = vec![Message::UserMessage {
            role: Some("user".to_string()),
            content: Some("Explain the importance of fast language models".to_string()),
            name: None,
            tool_call_id: None,
        }];
        let request =
            builder::RequestBuilder::new("mixtral-8x7b-32768".to_string()).with_stream(true);
        let api_key = env!("GROQ_API_KEY");

        let client = Groq::new(api_key);
        let mut client = client.add_messages(messages);

        let res = client.create(request).await;
        assert!(res.is_ok());
        println!("{:?}", res.unwrap());
        Ok(())
    }

    #[tokio::test]
    async fn error_does_return() -> anyhow::Result<()> {
        let messages = vec![Message::UserMessage {
            role: Some("user".to_string()),
            content: Some("Explain the importance of fast language models".to_string()),
            name: None,
            tool_call_id: None,
        }];
        let request =
            builder::RequestBuilder::new("mixtral-8x7b-32768".to_string()).with_stream(true);
        let api_key = "";

        let client = Groq::new(api_key);
        let mut client = client.add_messages(messages);

        let res = client.create(request).await;
        assert!(res.is_err());
        eprintln!("{}", res.unwrap_err());
        Ok(())
    }

    #[tokio::test]
    async fn create_with_add_tmp_message() -> anyhow::Result<()> {
        let messages = vec![Message::SystemMessage {
            content: Some("I am a system message".to_string()),
            name: None,
            role: Some("system".to_string()),
            tool_call_id: None,
        }];
        let request = builder::RequestBuilder::new("mixtral-8x7b-32768".to_string());
        let api_key = env!("GROQ_API_KEY");

        let client = Groq::new(api_key);
        let mut client = client
            .add_messages(messages)
            .add_tmp_message(Message::UserMessage {
                role: Some("user".to_string()),
                content: Some("Explain the importance of fast language models".to_string()),
                name: None,
                tool_call_id: None,
            });

        assert!(client.get_tmp_request_messages().is_some());
        let res = client.create(request).await;
        assert!(!res.is_err());
        assert!(client.get_tmp_request_messages().is_none());
        Ok(())
    }
}
