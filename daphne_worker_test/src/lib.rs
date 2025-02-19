// Copyright (c) 2022 Cloudflare, Inc. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use daphne_worker::{initialize_tracing, DaphneWorkerRouter};
use tracing::info;
use worker::*;

mod utils;

fn log_request(req: &Request) {
    info!(
        "[{}], located at: {:?}, within: {}",
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or_else(|| "unknown region".into())
    );
}

#[event(fetch, respond_with_errors)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    // Optionally, get more helpful error messages written to the console in the case of a panic.
    utils::set_panic_hook();

    // We set up logging as soon as possible so that logging can be estabished and functional
    // before we do anything likely to fail.
    initialize_tracing(&env);

    log_request(&req);

    let router = DaphneWorkerRouter {
        enable_internal_test: true,
        enable_default_response: false,
        ..Default::default()
    };
    router.handle_request(req, env).await
}
