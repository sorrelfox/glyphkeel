// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The local HTTP endpoint for agents whose client can't start a program:
//! MCP's Streamable HTTP transport at `http://127.0.0.1:<port>/mcp`.
//!
//! It is off unless the person turns it on (in the app's Preferences, or
//! with `glyphkeel mcp --http`). It listens on the loopback address only,
//! never makes a connection of its own, and answers only requests that
//!
//! - carry the token as `Authorization: Bearer <token>` (else `401`);
//! - name this endpoint in `Host` (`127.0.0.1:<port>` or
//!   `localhost:<port>`), so a web page can't reach it by pointing a
//!   name of its own at 127.0.0.1 (DNS rebinding);
//! - carry no `Origin`, or this endpoint's own, so a web page in a browser
//!   can't use it.
//!
//! Agents get the same tools, profiles and limits as over standard input
//! and output. `/mcp` serves the profile the endpoint was started with;
//! `/mcp/read-only`, `/mcp/standard` and `/mcp/full` serve that profile,
//! as `--profile` does. The workspace setting caps each. An old-era client's HTTP session is one agent session; a
//! 2026-07-28 client sends each request on its own, and its requests share
//! one agent session per client name and version until it has been idle
//! for [`IDLE`].

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr, TcpListener as StdListener};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use bytes::Bytes;
use gk_api::{OpError, Profile};
use gk_engine::Engine;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CompleteRequestParams, CompleteResult, DiscoverResult,
    GetPromptRequestParams, GetPromptResponse, Implementation, InitializeRequestParams,
    InitializeResult, ListPromptsResult, ListResourceTemplatesResult, ListResourcesResult,
    ListToolsResult, PaginatedRequestParams, ProtocolVersion, ReadResourceRequestParams,
    ReadResourceResponse, ServerConfig, SubscribeRequestParams, SubscriptionFilter, Tool,
    UnsubscribeRequestParams,
};
use rmcp::service::{RequestContext, SubscriptionContext};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use tokio_util::sync::CancellationToken;
use tower_service::Service as _;

use crate::messages;
use crate::random::same_secret;
use crate::resources::Feed;
use crate::server::{Server, Setup};

/// The path of the endpoint.
pub(crate) const PATH: &str = "/mcp";
/// A 2026-07-28 client's agent session ends after this long without a
/// request.
pub(crate) const IDLE: Duration = Duration::from_secs(30 * 60);
/// How often idle sessions are ended and, for `glyphkeel mcp --http`, the
/// background work is done.
const TICK: Duration = Duration::from_secs(1);
/// The largest request body.
const MAX_BODY: usize = crate::MAX_MESSAGE_BYTES;

type Body = BoxBody<Bytes, Infallible>;
type Service = StreamableHttpService<Router, LocalSessionManager>;

/// The running endpoint. Dropping it stops it.
#[derive(Debug)]
pub struct HttpEndpoint {
    addr: SocketAddr,
    stop: CancellationToken,
    thread: Option<JoinHandle<()>>,
}

impl HttpEndpoint {
    /// Starts serving `engine` to agents set up as `setup`, on 127.0.0.1
    /// at `port` (0: a free port), to clients that send `token`. With
    /// `background`, it also does the workspace's background work and
    /// stops when a newer program upgrades the workspace, as `glyphkeel
    /// mcp` does; the app does that work itself.
    ///
    /// # Errors
    /// `conflict` when the port is taken, `storage` when the endpoint
    /// can't start otherwise, `invalid` for an empty token.
    pub fn start(
        engine: Arc<Engine>,
        setup: Setup,
        port: u16,
        token: String,
        background: bool,
    ) -> Result<Self, OpError> {
        if token.trim().is_empty() {
            return Err(messages::error("mcp-http-no-token", &[], OpError::invalid));
        }
        let listener = StdListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|e| {
            let args = [("port", port.to_string()), ("error", e.to_string())];
            if e.kind() == std::io::ErrorKind::AddrInUse {
                messages::error("mcp-http-port-taken", &args, OpError::conflict)
            } else {
                messages::error("mcp-http-failed", &args, OpError::storage)
            }
        })?;
        let failed = |e: std::io::Error| {
            messages::error(
                "mcp-http-failed",
                &[("port", port.to_string()), ("error", e.to_string())],
                OpError::storage,
            )
        };
        let addr = listener.local_addr().map_err(failed)?;
        listener.set_nonblocking(true).map_err(failed)?;
        let stop = CancellationToken::new();
        // In the app, the engine publishes other programs' changes already.
        let feed = Arc::new(Feed::new(background));
        let mut routes = vec![(
            PATH.to_owned(),
            Arc::new(Shared::new(
                Arc::clone(&engine),
                setup.clone(),
                Arc::clone(&feed),
            )),
        )];
        for profile in [Profile::ReadOnly, Profile::Standard, Profile::Full] {
            let setup = Setup {
                profile,
                ..setup.clone()
            };
            routes.push((
                format!("{PATH}/{profile}"),
                Arc::new(Shared::new(Arc::clone(&engine), setup, Arc::clone(&feed))),
            ));
        }
        let thread = std::thread::Builder::new()
            .name("http-endpoint".to_owned())
            .spawn({
                let stop = stop.clone();
                move || run(listener, routes, token, stop, background)
            })
            .map_err(failed)?;
        tracing::info!(%addr, "the agents' HTTP endpoint is listening");
        Ok(Self {
            addr,
            stop,
            thread: Some(thread),
        })
    }

    /// The address it listens on (always 127.0.0.1).
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The URL clients connect to.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}{PATH}", self.addr.port())
    }

    /// Waits until it stops by itself: the workspace was upgraded by a newer
    /// program, or a stop signal came.
    pub fn wait(mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    /// Stops it and ends its agent sessions.
    pub fn stop(mut self) {
        self.shut_down();
    }

    /// A handle that stops it from another thread.
    #[must_use]
    pub fn stopper(&self) -> CancellationToken {
        self.stop.clone()
    }

    fn shut_down(&mut self) {
        self.stop.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for HttpEndpoint {
    fn drop(&mut self) {
        self.shut_down();
    }
}

/// The endpoint's thread: its own runtime, until stopped.
/// The paths and what each serves.
type Routes = Vec<(String, Arc<Shared>)>;

fn run(
    listener: StdListener,
    routes: Routes,
    token: String,
    stop: CancellationToken,
    background: bool,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            tracing::error!(error = %e, "starting the HTTP endpoint's runtime failed");
            return;
        }
    };
    runtime.block_on(accept(listener, routes.clone(), token, stop, background));
    for (_, shared) in &routes {
        shared.end_all();
    }
    // Dropping the runtime drops the open connections and with them the
    // old era's sessions, which end their agent sessions.
    drop(runtime);
}

async fn accept(
    listener: StdListener,
    routes: Routes,
    token: String,
    stop: CancellationToken,
    background: bool,
) {
    let listener = match tokio::net::TcpListener::from_std(listener) {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!(error = %e, "the HTTP endpoint can't listen");
            return;
        }
    };
    let port = listener.local_addr().map_or(0, |a| a.port());
    let config = StreamableHttpServerConfig::default()
        .with_allowed_hosts([format!("127.0.0.1:{port}"), format!("localhost:{port}")])
        .with_allowed_origins([
            format!("http://127.0.0.1:{port}"),
            format!("http://localhost:{port}"),
        ])
        .enforce_origin_validation()
        .with_max_request_body_bytes(MAX_BODY)
        .with_cancellation_token(stop.child_token());
    let services: Arc<Vec<(String, Service)>> = Arc::new(
        routes
            .iter()
            .map(|(path, shared)| {
                let shared = Arc::clone(shared);
                let service = StreamableHttpService::new(
                    move || Ok(Router::new(Arc::clone(&shared))),
                    Arc::new(LocalSessionManager::default()),
                    config.clone(),
                );
                (path.clone(), service)
            })
            .collect(),
    );
    let engine = Arc::clone(&routes[0].1.engine);
    let token = Arc::new(token);
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            () = stop.cancelled() => break,
            accepted = listener.accept() => {
                let (stream, peer) = match accepted {
                    Ok(accepted) => accepted,
                    Err(e) => {
                        tracing::warn!(error = %e, "accepting a connection failed");
                        continue;
                    }
                };
                // Bound to 127.0.0.1, so only this computer connects; checked
                // anyway.
                if !peer.ip().is_loopback() {
                    continue;
                }
                let services = Arc::clone(&services);
                let token = Arc::clone(&token);
                tokio::spawn(async move {
                    let handler = hyper::service::service_fn(move |request| {
                        let services = Arc::clone(&services);
                        let token = Arc::clone(&token);
                        async move { Ok::<_, Infallible>(answer(&services, &token, request).await) }
                    });
                    if let Err(e) = hyper::server::conn::http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), handler)
                        .await
                    {
                        tracing::debug!(error = %e, "an HTTP connection ended with an error");
                    }
                });
            }
            _ = tick.tick() => {
                for (_, shared) in &routes {
                    shared.end_idle(Instant::now());
                }
                if background {
                    if let Err(e) = engine.jobs_tick() {
                        tracing::warn!(error = %e, "background work failed");
                    }
                    if engine.must_exit() {
                        tracing::warn!("a newer program upgraded the workspace; stopping");
                        break;
                    }
                }
            }
        }
    }
}

/// One request: the path and the token first, then rmcp's service, which
/// checks `Host` and `Origin` and speaks the protocol.
async fn answer(
    services: &[(String, Service)],
    token: &str,
    request: Request<Incoming>,
) -> Response<Body> {
    let Some(mut service) = services
        .iter()
        .find(|(path, _)| path == request.uri().path())
        .map(|(_, service)| service.clone())
    else {
        return plain(StatusCode::NOT_FOUND, "Not found");
    };
    let sent = request
        .headers()
        .get(hyper::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim);
    if !sent.is_some_and(|sent| same_secret(sent.as_bytes(), token.as_bytes())) {
        let mut response = plain(StatusCode::UNAUTHORIZED, "Unauthorized");
        response.headers_mut().insert(
            hyper::header::WWW_AUTHENTICATE,
            hyper::header::HeaderValue::from_static("Bearer"),
        );
        return response;
    }
    match service.call(request).await {
        Ok(response) => response,
        Err(never) => match never {},
    }
}

fn plain(status: StatusCode, text: &'static str) -> Response<Body> {
    let mut response = Response::new(Full::new(Bytes::from_static(text.as_bytes())).boxed());
    *response.status_mut() = status;
    response.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

/// What every request of the endpoint shares.
#[derive(Debug)]
struct Shared {
    engine: Arc<Engine>,
    setup: Setup,
    feed: Arc<Feed>,
    /// The agent sessions of 2026-07-28 clients, by client.
    stateless: Mutex<HashMap<String, (Server, Instant)>>,
    /// A server that never starts a session, for questions without a
    /// client (the tool definitions).
    template: Server,
}

impl Shared {
    fn new(engine: Arc<Engine>, setup: Setup, feed: Arc<Feed>) -> Self {
        let template = Server::shared(Arc::clone(&engine), &setup, Arc::clone(&feed));
        Self {
            engine,
            setup,
            feed,
            stateless: Mutex::new(HashMap::new()),
            template,
        }
    }

    fn server(&self) -> Server {
        Server::shared(
            Arc::clone(&self.engine),
            &self.setup,
            Arc::clone(&self.feed),
        )
    }

    /// The agent session of a 2026-07-28 `client`.
    fn stateless(&self, client: Option<Implementation>) -> Server {
        let key = client.map_or_else(String::new, |c| format!("{}\u{1f}{}", c.name, c.version));
        let mut sessions = self
            .stateless
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let entry = sessions
            .entry(key)
            .or_insert_with(|| (self.server(), Instant::now()));
        entry.1 = Instant::now();
        entry.0.clone()
    }

    /// Ends the 2026-07-28 sessions idle for [`IDLE`].
    fn end_idle(&self, now: Instant) {
        let mut sessions = self
            .stateless
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        sessions.retain(|_, (server, last)| {
            let idle = now.duration_since(*last) >= IDLE;
            if idle {
                server.end();
            }
            !idle
        });
    }

    fn end_all(&self) {
        let sessions = std::mem::take(
            &mut *self
                .stateless
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        );
        for (server, _) in sessions.into_values() {
            server.end();
        }
    }
}

/// The handler rmcp makes for each old-era HTTP session and for each
/// 2026-07-28 request: it passes every request to the right agent session.
#[derive(Debug, Clone)]
struct Router {
    shared: Arc<Shared>,
    own: Arc<Own>,
}

/// An old-era HTTP session's own agent session, ended when rmcp drops the
/// session.
#[derive(Debug, Default)]
struct Own(OnceLock<Server>);

impl Drop for Own {
    fn drop(&mut self) {
        if let Some(server) = self.0.get() {
            server.end();
        }
    }
}

impl Router {
    fn new(shared: Arc<Shared>) -> Self {
        Self {
            shared,
            own: Arc::new(Own::default()),
        }
    }

    fn pick(&self, context: &RequestContext<RoleServer>) -> Server {
        match self.own.0.get() {
            Some(server) => server.clone(),
            None => self.shared.stateless(context.client_info()),
        }
    }
}

impl ServerHandler for Router {
    fn get_info(&self) -> ServerConfig {
        self.shared.template.get_info()
    }

    fn supported_protocol_versions(&self) -> std::borrow::Cow<'static, [ProtocolVersion]> {
        self.shared.template.supported_protocol_versions()
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        let server = self.own.0.get_or_init(|| self.shared.server()).clone();
        server.initialize(request, context).await
    }

    async fn discover(
        &self,
        context: RequestContext<RoleServer>,
    ) -> Result<DiscoverResult, McpError> {
        self.pick(&context).discover(context).await
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        self.pick(&context).list_tools(request, context).await
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        match self.own.0.get() {
            Some(server) => server.get_tool(name),
            None => self.shared.template.get_tool(name),
        }
    }

    async fn list_prompts(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        self.pick(&context).list_prompts(request, context).await
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        self.pick(&context).get_prompt(request, context).await
    }

    async fn complete(
        &self,
        request: CompleteRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CompleteResult, McpError> {
        self.pick(&context).complete(request, context).await
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        self.pick(&context).call_tool(request, context).await
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        self.pick(&context).list_resources(request, context).await
    }

    async fn list_resource_templates(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        self.pick(&context)
            .list_resource_templates(request, context)
            .await
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        self.pick(&context).read_resource(request, context).await
    }

    #[allow(deprecated)]
    async fn subscribe(
        &self,
        request: SubscribeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        self.pick(&context).subscribe(request, context).await
    }

    #[allow(deprecated)]
    async fn unsubscribe(
        &self,
        request: UnsubscribeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        self.pick(&context).unsubscribe(request, context).await
    }

    fn accepted_subscription_filter(
        &self,
        requested: &SubscriptionFilter,
    ) -> Option<SubscriptionFilter> {
        self.shared.template.accepted_subscription_filter(requested)
    }

    async fn listen(&self, context: SubscriptionContext) -> Result<(), McpError> {
        let server = self.pick(context.request_context());
        server.listen(context).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::ClientInfo;
    use gk_api::ops::common::Via;
    use gk_engine::Context;
    use serde_json::{Value, json};

    /// The endpoint's shared state over a new workspace, as `/mcp` has it.
    fn shared() -> (tempfile::TempDir, Shared) {
        let dir = tempfile::tempdir().unwrap();
        let mut options = crate::Options::with_workspace(dir.path().join("Test").to_str().unwrap());
        options.settings_file = Some(dir.path().join("settings.json"));
        let engine = Arc::new(crate::open_engine(&options).unwrap());
        let shared = Shared::new(engine, options.setup(), Arc::new(Feed::new(false)));
        (dir, shared)
    }

    fn client(n: usize) -> Implementation {
        Implementation::new(format!("client-{n}"), "1.0")
    }

    /// Starts the agent session of `client`'s server, as its first call does.
    fn start(shared: &Shared, n: usize) -> Server {
        let server = shared.stateless(Some(client(n)));
        let info = ClientInfo {
            name: format!("client-{n}"),
            version: "1.0".to_owned(),
            title: None,
        };
        server
            .session()
            .context(&shared.engine, Some(&info))
            .unwrap();
        server
    }

    fn sessions(shared: &Shared) -> Vec<Value> {
        let all = shared
            .engine
            .call(
                &Context::user(Via::App),
                "agent.sessions",
                json!({ "limit": 500 }),
            )
            .unwrap();
        all["items"].as_array().unwrap().clone()
    }

    fn open_count(shared: &Shared) -> usize {
        sessions(shared)
            .iter()
            .filter(|s| s["ended_at"].is_null())
            .count()
    }

    #[test]
    fn an_idle_2026_session_ends_after_the_idle_time_and_a_new_request_starts_another() {
        let (_dir, shared) = shared();
        let first = start(&shared, 1);
        let started = Instant::now();
        assert_eq!(open_count(&shared), 1);

        // Just short of the idle time it stays.
        shared.end_idle(started + IDLE - Duration::from_secs(1));
        assert_eq!(open_count(&shared), 1);
        let again = shared.stateless(Some(client(1)));
        assert_eq!(again.session().id(), first.session().id());

        // A request refreshed it: the idle time counts from there.
        let refreshed = Instant::now();
        shared.end_idle(refreshed + IDLE - Duration::from_secs(1));
        assert_eq!(open_count(&shared), 1);

        shared.end_idle(refreshed + IDLE);
        assert_eq!(open_count(&shared), 0);
        let all = sessions(&shared);
        assert_eq!(all.len(), 1);
        assert!(all[0]["ended_at"].is_string(), "{}", all[0]);

        // The same client comes back later: a new agent session.
        let later = start(&shared, 1);
        assert_ne!(later.session().id(), first.session().id());
        assert_eq!(sessions(&shared).len(), 2);
        assert_eq!(open_count(&shared), 1);
        shared.end_all();
        assert_eq!(open_count(&shared), 0);
    }

    #[test]
    fn many_clients_have_a_session_each_and_only_the_idle_ones_end() {
        let (_dir, shared) = shared();
        let clients = 150;
        let servers: Vec<Server> = (0..clients).map(|n| start(&shared, n)).collect();
        let ids: std::collections::BTreeSet<_> =
            servers.iter().map(|s| s.session().id().unwrap()).collect();
        assert_eq!(ids.len(), clients);
        assert_eq!(open_count(&shared), clients);

        // Every client asks again: still one session each.
        for (n, server) in servers.iter().enumerate() {
            let again = shared.stateless(Some(client(n)));
            assert_eq!(again.session().id(), server.session().id());
        }
        assert_eq!(sessions(&shared).len(), clients);

        // The even ones keep asking; the odd ones fall idle.
        let now = Instant::now();
        {
            let mut stateless = shared.stateless.lock().unwrap();
            for (_, last) in stateless.values_mut() {
                *last = now;
            }
            for n in (1..clients).step_by(2) {
                let key = format!("client-{n}\u{1f}1.0");
                stateless.get_mut(&key).unwrap().1 = now - Duration::from_secs(60);
            }
        }
        shared.end_idle(now + IDLE - Duration::from_secs(30));
        assert_eq!(open_count(&shared), clients / 2);
        assert_eq!(shared.stateless.lock().unwrap().len(), clients / 2);
        shared.end_all();
        assert_eq!(open_count(&shared), 0);
        assert!(shared.stateless.lock().unwrap().is_empty());
    }
}
