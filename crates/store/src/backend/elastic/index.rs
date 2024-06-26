/*
 * Copyright (c) 2023 Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart Mail Server.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
*/

use std::{borrow::Cow, fmt::Display};

use elasticsearch::{DeleteByQueryParts, IndexParts};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    backend::elastic::INDEX_NAMES,
    dispatch::DocumentSet,
    fts::{index::FtsDocument, Field},
};

use super::ElasticSearchStore;

#[derive(Serialize, Deserialize, Default)]
struct Document<'x> {
    document_id: u32,
    account_id: u32,
    body: Vec<Cow<'x, str>>,
    attachments: Vec<Cow<'x, str>>,
    keywords: Vec<Cow<'x, str>>,
    header: Vec<Header<'x>>,
}

#[derive(Serialize, Deserialize)]
struct Header<'x> {
    name: Cow<'x, str>,
    value: Cow<'x, str>,
}

impl ElasticSearchStore {
    pub async fn fts_index<T: Into<u8> + Display + Clone + std::fmt::Debug>(
        &self,
        document: FtsDocument<'_, T>,
    ) -> crate::Result<()> {
        self.index
            .index(IndexParts::Index(INDEX_NAMES[document.collection as usize]))
            .body(Document::from(document))
            .send()
            .await
            .map_err(Into::into)
            .and_then(|response| {
                if response.status_code().is_success() {
                    Ok(())
                } else {
                    Err(crate::Error::InternalError(format!(
                        "Failed to index document: {:?}",
                        response
                    )))
                }
            })
    }

    pub async fn fts_remove(
        &self,
        account_id: u32,
        collection: u8,
        document_ids: &impl DocumentSet,
    ) -> crate::Result<()> {
        let document_ids = document_ids.iterate().collect::<Vec<_>>();

        self.index
            .delete_by_query(DeleteByQueryParts::Index(&[
                INDEX_NAMES[collection as usize]
            ]))
            .body(json!({
                "query": {
                    "bool": {
                        "must": [
                            { "match": { "account_id": account_id } },
                            { "terms": { "document_id": document_ids } }
                        ]
                    }
                }
            }))
            .send()
            .await
            .map_err(Into::into)
            .and_then(|response| {
                if response.status_code().is_success() {
                    Ok(())
                } else {
                    Err(crate::Error::InternalError(format!(
                        "Failed to remove document: {:?}",
                        response
                    )))
                }
            })
    }

    pub async fn fts_remove_all(&self, account_id: u32) -> crate::Result<()> {
        self.index
            .delete_by_query(DeleteByQueryParts::Index(INDEX_NAMES))
            .body(json!({
                "query": {
                    "bool": {
                        "must": [
                            { "match": { "account_id": account_id } },
                        ]
                    }
                }
            }))
            .send()
            .await
            .map_err(Into::into)
            .and_then(|response| {
                if response.status_code().is_success() {
                    Ok(())
                } else {
                    Err(crate::Error::InternalError(format!(
                        "Failed to remove document: {:?}",
                        response
                    )))
                }
            })
    }
}

impl<'x, T: Into<u8> + Display + Clone + std::fmt::Debug> From<FtsDocument<'x, T>>
    for Document<'x>
{
    fn from(value: FtsDocument<'x, T>) -> Self {
        let mut document = Document {
            account_id: value.account_id,
            document_id: value.document_id,
            ..Default::default()
        };

        for part in value.parts {
            match part.field {
                Field::Header(name) => document.header.push(Header {
                    name: name.to_string().into(),
                    value: part.text,
                }),
                Field::Body => document.body.push(part.text),
                Field::Attachment => document.attachments.push(part.text),
                Field::Keyword => document.keywords.push(part.text),
            }
        }

        document
    }
}
