// Copyright (c) 2022 Cloudflare, Inc. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! DAP request authorization.

use crate::{
    constants::DapMediaType,
    messages::{constant_time_eq, TaskId},
    DapError, DapRequest, DapSender,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A bearer token used for authorizing DAP requests.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BearerToken {
    raw: String,
}

impl AsRef<str> for BearerToken {
    fn as_ref(&self) -> &str {
        self.raw.as_str()
    }
}

impl PartialEq for BearerToken {
    fn eq(&self, other: &Self) -> bool {
        constant_time_eq(self.raw.as_bytes(), other.raw.as_bytes())
    }
}

impl From<String> for BearerToken {
    fn from(raw: String) -> Self {
        Self { raw }
    }
}

impl From<&str> for BearerToken {
    fn from(raw: &str) -> Self {
        Self {
            raw: raw.to_string(),
        }
    }
}

impl AsRef<BearerToken> for BearerToken {
    fn as_ref(&self) -> &Self {
        self
    }
}

/// A source of bearer tokens used for authorizing DAP requests.
#[async_trait(?Send)]
pub trait BearerTokenProvider<'a> {
    /// A reference to a bearer token owned by the provider.
    type WrappedBearerToken: AsRef<BearerToken>;

    /// Fetch the Leader's bearer token for the given task, if the task is recognized.
    async fn get_leader_bearer_token_for(
        &'a self,
        task_id: &'a TaskId,
    ) -> Result<Option<Self::WrappedBearerToken>, DapError>;

    /// Fetch the Collector's bearer token for the given task, if the task is recognized.
    async fn get_collector_bearer_token_for(
        &'a self,
        task_id: &'a TaskId,
    ) -> Result<Option<Self::WrappedBearerToken>, DapError>;

    /// Returns true if the given bearer token matches the leader token configured for the "taskprov" extension.
    fn is_taskprov_leader_bearer_token(&self, token: &BearerToken) -> bool;

    /// Returns true if the given bearer token matches the collector token configured for the "taskprov" extension.
    fn is_taskprov_collector_bearer_token(&self, token: &BearerToken) -> bool;

    /// Return a bearer token that can be used to authorize a request with the given task ID and
    /// media type.
    async fn authorize_with_bearer_token(
        &'a self,
        task_id: &'a TaskId,
        media_type: &DapMediaType,
    ) -> Result<Self::WrappedBearerToken, DapError> {
        if matches!(media_type.sender(), Some(DapSender::Leader)) {
            let token = self
                .get_leader_bearer_token_for(task_id)
                .await?
                .ok_or_else(|| {
                    DapError::Fatal("attempted to authorize request with unknown task ID".into())
                })?;
            return Ok(token);
        }

        Err(DapError::Fatal(format!(
            "attempted to authorize request of type '{media_type:?}'",
        )))
    }

    /// Check that the bearer token carried by a request can be used to authorize that request.
    ///
    /// Return `None` if the request is authorized. Otherwise return `Some(reason)`, where `reason`
    /// is the reason for the failure.
    async fn bearer_token_authorized<T: AsRef<BearerToken>>(
        &'a self,
        req: &'a DapRequest<T>,
    ) -> Result<Option<String>, DapError> {
        if req.task_id.is_none() {
            // Can't authorize request with missing task ID.
            return Ok(Some(
                "Cannot authorize request with missing task ID.".into(),
            ));
        }
        let task_id = req.task_id.as_ref().unwrap();

        // TODO spec: Decide whether to check that the bearer token has the right format, say,
        // following RFC 6750, Section 2.1. Note that we would also need to replace `From<String>
        // for BearerToken` with `TryFrom<String>` so that a `DapError` can be returned if the
        // token is not formatted properly.
        if matches!(req.media_type.sender(), Some(DapSender::Leader)) {
            if let Some(ref got) = req.sender_auth {
                if let Some(expected) = self.get_leader_bearer_token_for(task_id).await? {
                    return Ok(if got.as_ref() == expected.as_ref() {
                        None
                    } else {
                        Some("The indicated beareer token is incorrect for the Leader.".into())
                    });
                }
                return Ok(if self.is_taskprov_leader_bearer_token(got.as_ref()) {
                    None
                } else {
                    Some("The indicated beaer token is incorrect for Taskprov Leader.".into())
                });
            }
        }

        if matches!(req.media_type.sender(), Some(DapSender::Collector)) {
            if let Some(ref got) = req.sender_auth {
                if let Some(expected) = self.get_collector_bearer_token_for(task_id).await? {
                    return Ok(if got.as_ref() == expected.as_ref() {
                        None
                    } else {
                        Some("The indicated bearer token is incorrect for the Collector.".into())
                    });
                }
                return Ok(if self.is_taskprov_collector_bearer_token(got.as_ref()) {
                    None
                } else {
                    Some(
                        "The indicated bearer token is incorrect for the Taskprov Collector."
                            .into(),
                    )
                });
            }
        }

        // Deny request with unhandled or unknown media type.
        Ok(Some(format!(
            "Cannot resolve sender due to unexpected media type ({:?}).",
            req.media_type
        )))
    }
}
