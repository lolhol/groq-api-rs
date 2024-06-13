use super::message::Message;
use serde::Serialize;
pub mod builder;

#[derive(Debug, Serialize)]
pub struct Request {
    // unused for openai integration only
    #[serde(skip_serializing_if = "Option::is_none")]
    logit_bias: Option<serde_json::Value>,

    // unused for openai integration only
    logprobs: bool,         // default false
    frequency_penalty: f32, // defaults to 0
    //
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,

    messages: Vec<Message>,
    model: String,

    n: u32,                          // defaults to 1
    presence_penalty: f32,           // defaults to 0
    response_format: ResponseFormat, // defaults to text,

    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<StopEnum>,

    stream: bool,     // default false
    temperature: f32, // defaults to 1

    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ToolChoiceEnum>,

    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Tool>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    top_logprobs: Option<u8>,

    top_p: f32, // defaults to 1

    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
}
impl Request {
    pub fn is_stream(&self) -> bool {
        self.stream
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ToolChoiceEnum {
    Str(String),
    Tool(Tool),
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum StopEnum {
    Token(String),
    Tokens(Vec<String>),
}

#[derive(Debug, Serialize)]
pub struct Tool {
    #[serde(rename(serialize = "type"))]
    pub tool_type: String,
    pub function: Function,
}

#[derive(Debug, Serialize)]
pub struct Function {
    pub description: Option<String>,
    pub name: Option<String>,
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ResponseFormat {
    #[serde(rename(serialize = "type"))]
    pub response_type: String,
}

#[cfg(test)]
mod request_test {
    use crate::completion::request::*;
    use anyhow::Context;

    #[test]
    fn init_request() -> anyhow::Result<()> {
        let messages = vec![Message::UserMessage {
            content: None,
            name: None,
            role: None,
            tool_call_id: None,
        }];
        let target = Request {
            logit_bias: None,
            logprobs: false,
            frequency_penalty: 0.0,
            max_tokens: None,
            messages: messages.clone(),
            model: "".into(),
            n: 1,
            presence_penalty: 0.0,
            response_format: ResponseFormat {
                response_type: "text".into(),
            },
            seed: None,
            stop: None,
            stream: false,
            temperature: 1.0,
            tool_choice: None,
            tools: None,
            top_logprobs: None,
            top_p: 1.0,
            user: None,
        };
        let req2 = builder::RequestBuilder::new("".into(), messages)
            .context("the message vec should contain at least 1 Message")?
            .build();

        assert_eq!(
            serde_json::to_string(&target).unwrap(),
            serde_json::to_string(&req2).unwrap()
        );
        Ok(())
    }

    #[test]
    fn with_stop_enum() -> anyhow::Result<()> {
        let messages = vec![Message::UserMessage {
            content: None,
            name: None,
            role: None,
            tool_call_id: None,
        }];
        let mut target = Request {
            logit_bias: None,
            logprobs: false,
            frequency_penalty: 0.0,
            max_tokens: None,
            messages: messages.clone(),
            model: "".into(),
            n: 1,
            presence_penalty: 0.0,
            response_format: ResponseFormat {
                response_type: "text".into(),
            },
            seed: None,
            stop: Some(StopEnum::Token("endline".into())),
            stream: false,
            temperature: 1.0,
            tool_choice: None,
            tools: None,
            top_logprobs: None,
            top_p: 1.0,
            user: None,
        };
        let req2 = builder::RequestBuilder::new("".to_string(), messages.clone())
            .context("the messages vec should be at least 1")?
            .with_stop("endline")
            .build();

        let out_json = serde_json::to_string(&req2).unwrap();
        assert_eq!(serde_json::to_string(&target).unwrap(), out_json);
        let stops = vec!["endline".to_string()];
        target.stop = Some(StopEnum::Tokens(stops.clone()));
        let req2 = builder::RequestBuilder::new("".into(), messages)
            .context("the message vec should contain at least 1 Message")?
            .with_stops(stops)
            .build();

        let out_json = serde_json::to_string(&req2).unwrap();
        assert_eq!(serde_json::to_string(&target).unwrap(), out_json);
        Ok(())
    }

    #[test]
    fn with_messages() -> anyhow::Result<()> {
        let messages = vec![
            Message::UserMessage {
                content: None,
                name: None,
                role: None,
                tool_call_id: None,
            },
            Message::SystemMessage {
                content: None,
                name: None,
                role: None,
                tool_call_id: None,
            },
            Message::AssistantMessage {
                content: None,
                name: None,
                role: None,
                tool_call_id: None,
                tool_calls: None,
            },
            Message::ToolMessage {
                content: None,
                name: None,
                role: None,
                tool_call_id: None,
            },
        ];
        let target = Request {
            logit_bias: None,
            logprobs: false,
            frequency_penalty: 0.0,
            max_tokens: None,
            messages: messages.clone(),
            model: "".into(),
            n: 1,
            presence_penalty: 0.0,
            response_format: ResponseFormat {
                response_type: "text".into(),
            },
            seed: None,
            stop: None,
            stream: false,
            temperature: 1.0,
            tool_choice: None,
            tools: None,
            top_logprobs: None,
            top_p: 1.0,
            user: None,
        };
        let req2 = builder::RequestBuilder::new("".to_string(), messages)
            .context("messages should be inserted to builder")?
            .build();

        let target_json = serde_json::to_string(&target).unwrap();
        let out_json = serde_json::to_string(&req2).unwrap();
        assert_eq!(target_json, out_json);
        Ok(())
    }
}
