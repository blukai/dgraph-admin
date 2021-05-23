use anyhow::{anyhow, Context, Result};
use argh::FromArgs;
use humantime::format_duration;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::{fs, time::Duration};
use url::Url;

struct Dgraph {
    base_url: String,
    auth_header: Option<String>,
}

#[derive(Serialize, Debug)]
struct GqlRequest<'q, Variables> {
    query: &'q str,
    variables: Variables,
}

#[derive(Deserialize, Debug)]
struct GqlError {
    message: String,
}

// spec: https://spec.graphql.org/June2018/#sec-Response-Format
#[derive(Deserialize, Debug)]
struct GqlResponse<Data> {
    data: Option<Data>,
    errors: Option<Vec<GqlError>>,
}

impl Dgraph {
    // `new` constructs a new client for dgraph.
    // if the given url is missing a scheme (such as "http://" or "https://")
    // then "http://" will be used.
    // url must not end with `/graphql`, if it is - it will be trimmed off.
    fn new(url: String, auth_header: Option<String>) -> Result<Self> {
        // if scheme is not provided (example: localhost:8080)
        // then host may be parsed as scheme,
        // see: https://github.com/servo/rust-url/issues/613
        let mut parsed_url = Url::parse(&url)?;
        // add scheme, if missing
        let scheme = parsed_url.scheme();
        if !(scheme == "http" || scheme == "https") {
            let schemeful_url = "http://".to_string() + &url;
            parsed_url = Url::parse(&schemeful_url).context("your url is fucky wacky. sorry!")?;
        }
        // trim off path
        parsed_url.set_path("");

        Ok(Self {
            base_url: parsed_url.to_string(),
            auth_header,
        })
    }

    fn add_auth_header(&self, req: ureq::Request) -> ureq::Request {
        if let Some(ah) = &self.auth_header {
            if let Some((key, value)) = ah.split_once(':') {
                return req.set(key, value);
            }
        }
        req
    }

    fn post(&self, endpoint: &str) -> ureq::Request {
        let req = ureq::post(&(self.base_url.clone() + endpoint));
        self.add_auth_header(req)
    }

    fn get(&self, endpoint: &str) -> ureq::Request {
        let req = ureq::get(&(self.base_url.clone() + endpoint));
        self.add_auth_header(req)
    }

    fn query<Variables: Serialize, Data: DeserializeOwned>(
        &self,
        endpoint: &str,
        query: &str,
        variables: Variables,
    ) -> Result<Option<Data>> {
        let resp: GqlResponse<Data> = self
            .post(endpoint)
            .send_json(json!(GqlRequest { query, variables }))?
            .into_json()?;
        if let Some(errors) = resp.errors {
            // gql errors can be prettier, but do i expect to see them often
            // to care enough? no.
            Err(anyhow!("{:#?}", &errors))
        } else {
            Ok(resp.data)
        }
    }

    fn alter(&self, payload: &str) -> Result<()> {
        let resp = self.post("alter").send_string(payload)?.into_string()?;
        // https://dgraph.io/docs/clients/raw-http/#alter-the-database says to
        // expect `{"code":"Success","message":"Done"}`, but in fact the
        // response is a little bit different
        if resp != r#"{"data":{"code":"Success","message":"Done"}}"# {
            Err(anyhow!("unexpected response: {:?}", &resp))
        } else {
            Ok(())
        }
    }
}

#[derive(FromArgs)]
#[argh(
    subcommand,
    name = "update-schema",
    description = "add or modify schema"
)]
struct UpdateSchema {
    #[argh(positional)]
    file: String,
}
impl UpdateSchema {
    fn exec(self, dgraph: &Dgraph) -> Result<()> {
        let schema = fs::read_to_string(self.file)?;
        let _ = dgraph.query::<JsonValue, JsonValue>(
            "admin",
            r#"mutation updateGQLSchema($schema: String!) {
                updateGQLSchema(input: { set: { schema: $schema } }) {
                    gqlSchema { id }
                }
            }"#,
            json!({ "schema": &schema }),
        )?;
        println!("success");
        Ok(())
    }
}

#[derive(FromArgs)]
#[argh(
    subcommand,
    name = "get-schema",
    description = "get the current schema"
)]
struct GetSchema {}
impl GetSchema {
    fn exec(self, dgraph: &Dgraph) -> Result<()> {
        let resp = dgraph.query::<(), JsonValue>(
            "admin",
            r#"query getGQLSchema {
                getGQLSchema { schema }
            }"#,
            (),
        )?;
        if let Some(data) = resp {
            // NOTE: schema is null on a new database, but if drop-all
            // was called - schema is ""(empty string).
            let schema = data["getGQLSchema"]["schema"]
                .as_str()
                .unwrap_or_default()
                .trim();
            if schema.is_empty() {
                println!("no schema");
            } else {
                println!("{}", schema);
            }
        }
        Ok(())
    }
}

#[derive(FromArgs)]
#[argh(
    subcommand,
    name = "drop-all",
    description = "drop all data and schema"
)]
struct DropAll {}
impl DropAll {
    fn exec(self, dgraph: &Dgraph) -> Result<()> {
        dgraph.alter(r#"{"drop_all": true}"#)?;
        println!("success");
        Ok(())
    }
}

#[derive(FromArgs)]
#[argh(
    subcommand,
    name = "drop-data",
    description = "drop all data only (keep schema)"
)]
struct DropData {}
impl DropData {
    fn exec(self, dgraph: &Dgraph) -> Result<()> {
        dgraph.alter(r#"{"drop_op": "DATA"}"#)?;
        println!("success");
        Ok(())
    }
}

#[derive(FromArgs)]
#[argh(subcommand, name = "get-health", description = "get status of nodes")]
struct GetHealth {}

#[derive(Deserialize, Debug)]
struct HealthResponse {
    // NOTE: this struct is not complete, it has only what's needed,
    // actual response contains more data.
    // see: https://dgraph.io/docs/graphql/admin
    address: String,
    status: String,
    uptime: u64,
}

impl GetHealth {
    fn exec(self, dgraph: &Dgraph) -> Result<()> {
        let resp: Vec<HealthResponse> = dgraph.get("health").call()?.into_json()?;
        for h in &resp {
            println!(
                "{} is {}, uptime: {}",
                &h.address,
                &h.status,
                format_duration(Duration::new(h.uptime, 0)),
            );
        }
        Ok(())
    }
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum SubCommand {
    UpdateSchema(UpdateSchema),
    GetSchema(GetSchema),
    DropAll(DropAll),
    DropData(DropData),
    GetHealth(GetHealth),
}
impl SubCommand {
    fn exec(self, dgraph: &Dgraph) -> Result<()> {
        match self {
            SubCommand::UpdateSchema(x) => x.exec(dgraph),
            SubCommand::GetSchema(x) => x.exec(dgraph),
            SubCommand::DropAll(x) => x.exec(dgraph),
            SubCommand::DropData(x) => x.exec(dgraph),
            SubCommand::GetHealth(x) => x.exec(dgraph),
        }
    }
}

#[derive(FromArgs)]
#[argh(description = "dgraph-admin is a simple tool for managing dgraph.")]
struct Args {
    #[argh(
        option,
        description = "dgraph url",
        default = "String::from(\"localhost:8080\")"
    )]
    url: String,

    #[argh(option, description = "auth header to include with the request")]
    auth: Option<String>,

    #[argh(subcommand)]
    subcommand: SubCommand,
}

fn main() -> Result<()> {
    let args: Args = argh::from_env();
    let dgraph = Dgraph::new(args.url, args.auth)?;
    args.subcommand.exec(&dgraph)
}
