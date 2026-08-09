use anyhow::{Result, Error};
use std::io::{self, Read};

/* example input
buf = "
--------------------------------------------------------------------------------

❓❓❓

give me a random English word

🌲🌲🌲🌲🌲🌲

Here's a random English word:


**Lugubrious**

*(adjective)*

Meaning: Looking or sounding sad and dismal; mournful.


Example: *\"The dog's lugubrious expression made everyone in the room feel a bit melancholic.\"*


Would you like another one?

➖

--------------------------------------------------------------------------------

❓❓❓

yes".to_owned();
*/

// -----------------------------------------------------------------------------
#[tokio::main]
async fn main() -> Result<()> {
	let msgs = list_messages()?;

	deepseek::api(msgs).await?;

	Ok(())
}

#[derive(Debug)]
pub struct Message {
	pub content: String,
	pub role: String,
}
fn list_messages() -> Result<Vec<Message>, Error> {
	let mut buf = String::new();
	io::stdin().read_to_string(&mut buf)?;

	let mut r = Vec::new();
	let mut a = &buf[0..];

	let pat_q = "\n\n❓❓❓\n\n";
	let pat_a = "\n\n🌲🌲🌲🌲🌲🌲\n\n";
	let pat_e = "\n\n➖\n\n";

	while a.len() > 0 {
		let Some(q_start) = a.find(pat_q) else {
			break;
		};
		a = &a[q_start + pat_q.len()..];

		let Some(a_start) = a.find(pat_a) else {
			// no answer
			r.push(Message {
				content: a[0..a.len()].trim_end().to_owned(),
				role: "user".to_owned(),
			});
			break;
		};
		r.push(Message {
			content: a[0..a_start].to_owned(),
			role: "user".to_owned(),
		});
		a = &a[a_start + pat_a.len()..];

		let Some(e_start) = a.find(pat_e) else {
			r.push(Message {
				content: a[0..a.len()].to_owned(),
				role: "assistant".to_owned(),
			});
			break;
		};
		r.push(Message {
			content: a[0..e_start].to_owned(),
			role: "assistant".to_owned(),
		});
		a = &a[e_start + pat_e.len()..];
	}

	if r.is_empty() {
		return Err(Error::msg("empty messages"));
	}
	if r[r.len() - 1].role != "user" {
		return Err(Error::msg("the role in the last message must be \"user\""));
	}
	if r[r.len() - 1].content.is_empty() {
		return Err(Error::msg("the last question must not be empty "));
	}
	return Ok(r);
}

// -----------------------------------------------------------------------------
pub mod deepseek {
	use anyhow::{Result, Error};
	use reqwest::StatusCode;

	const URL_BASE: &str = "https://api.deepseek.com";
	const URL_COMPLETIONS: &str = "/chat/completions";

	pub async fn api(msgs: Vec<super::Message>) -> Result<()> {
		let model = "deepseek-v4-flash".to_owned();
		let thinking = Some(req::Thinking {
			typ: "disabled".to_owned(),
		});
		let reasoning_effort = "low".to_owned();
		let stream = false;
		let url_completions = format!("{}{}", URL_BASE, URL_COMPLETIONS);
		let messages = msgs.into_iter().map(|it| {
			req::Message{
				content: it.content,
				role: it.role,
			}
		}).collect();

		let req = req::Request {
			messages,
			model,
			thinking,
			reasoning_effort,
			stream,
			..req::Request::default()
		};

		// -------------------------------------------------------------------------
		let Some(key) = std::env::var("DEEPSEEK_API_KEY").ok() else {
			return Err(Error::msg("DEEPSEEK_API_KEY must be set"));
		};
		use reqwest::header;
		let mut headers = header::HeaderMap::new();
		let mut auth_value = header::HeaderValue::from_str(&format!("Bearer {}", key))?;
		auth_value.set_sensitive(true);
		headers.insert(header::AUTHORIZATION, auth_value);

		let client = reqwest::Client::builder()
			.default_headers(headers)
			.build()?;
		let resp = client
			.post(url_completions)
			.json(&req)
			.send().await?;

		let status = resp.status();
		if status != StatusCode::OK {
			let err = match status {
				StatusCode::BAD_REQUEST => "invalid request body format".to_owned(),
				StatusCode::UNAUTHORIZED => "auth failed, perhaps wrong API key".to_owned(),
				StatusCode::PAYMENT_REQUIRED => "insufficient balance".to_owned(),
				StatusCode::UNPROCESSABLE_ENTITY => "invalid request parameters".to_owned(),
				StatusCode::TOO_MANY_REQUESTS => "rate limit reached".to_owned(),
				StatusCode::INTERNAL_SERVER_ERROR => "server error".to_owned(),
				StatusCode::SERVICE_UNAVAILABLE => "server is overloaded".to_owned(),
				_ => format!("error code: {}", status)
			};
			return Err(Error::msg(err));
		}

		let resp: resp::Response = resp.json().await?;
		if resp.choices.is_empty() {
			return Err(Error::msg("empty resp choices"));
		}

		println!("{}", resp.choices[0].message.content);

		Ok(())
	}

	pub mod req {
		use serde::{Serialize};

		#[derive(Debug, Default, Serialize)]
		pub struct Request {
			pub messages: Vec<Message>,
			pub model: String,
			pub thinking: Option<Thinking>,
			pub reasoning_effort: String,
			pub max_tokens: Option<i64>,
			pub response_format: Option<ResponseFormat>,
			pub stream: bool,
			pub stream_options: Option<StreamOptions>,
			pub temperature: Option<i32>,
			pub top_p: Option<i32>,
			pub logprobs: bool,
			pub top_logprobs: Option<i32>,
		}

		#[derive(Debug, Default, Serialize)]
		pub struct Message {
			pub content: String,
			pub role: String,
		}
		#[derive(Debug, Default, Serialize)]
		pub struct Thinking {
			#[serde(rename = "type")]
			pub typ: String,
		}
		#[derive(Debug, Default, Serialize)]
		pub struct ResponseFormat {
			#[serde(rename = "type")]
			pub typ: String,
		}
		#[derive(Debug, Default, Serialize)]
		pub struct StreamOptions {
			pub include_usage: bool,
		}
	}

	pub mod resp {
		use serde::{Deserialize};

		#[derive(Debug, Default, Deserialize)]
		pub struct Response {
			pub id: String,
			pub model: String,
			pub object: String,
			pub created: i64,
			pub system_fingerprint: String,
			pub choices: Vec<Choice>,
			pub usage: Usage,
		}

		#[derive(Debug, Default, Deserialize)]
		pub struct Choice {
			pub finish_reason: String,
			pub index: i64,
			pub message: Message,
		}

		#[derive(Debug, Default, Deserialize)]
		pub struct Message {
			pub content: String,
			pub role: String,
			pub reasoning_content: Option<String>,
		}

		#[derive(Debug, Default, Deserialize)]
		pub struct Usage {
			pub completion_tokens: i64,
			pub prompt_tokens: i64,
			pub prompt_cache_hit_tokens: i64,
			pub prompt_cache_miss_tokens: i64,
			pub total_tokens: i64,
			pub completion_tokens_details: Option<CompletionTokensDetails>,
		}

		#[derive(Debug, Default, Deserialize)]
		pub struct CompletionTokensDetails {
			pub reasoning_tokens: i64,
		}
	}

}
