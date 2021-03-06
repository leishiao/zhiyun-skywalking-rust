// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements.  See the NOTICE file distributed with
// this work for additional information regarding copyright ownership.
// The ASF licenses this file to You under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License.  You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::config::Config;
// use crate::remote::*;
use crate::remote::trace_segment_report_service_client::TraceSegmentReportServiceClient;
use crate::remote::{KeyStringValuePair, SegmentObject, SegmentReference, SpanObject};
use crate::skywalking::core::{ContextListener, TracingContext};
use anyhow::Result;
use futures_util::stream;
use lazy_static::lazy_static;
use log::*;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::runtime::{Builder, Handle, Runtime};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::Sender;
use tokio::time::{sleep, timeout};
use tonic::transport::Channel;
use tonic::Code;
use tonic::Request;

// batch skywalking segments every REPORT_BUCKET microseconds
const REPORT_BUCKET: u128 = 500;
const RECONNECT_SECES: u64 = 30;

lazy_static! {
    static ref GLOBAL_RT: Arc<Runtime> = Arc::new(
        Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("create tokio runtime fail")
    );
    static ref GLOBAL_HANDLE: Arc<Handle> = Arc::new(GLOBAL_RT.handle().clone());
}

#[allow(dead_code)]
pub struct Reporter {
    sender: Sender<Box<TracingContext>>,
    pub config: Config,
}

impl ContextListener for Reporter {
    // ????????????ipv4??????????????????instance_id
    fn service_instance_id(&self) -> Option<i32> {
        Some(self.config.instance_id)
    }

    // ??????????????????,????????????,????????????+1
    fn report_trace(&self, trace_info: Box<TracingContext>, try_times: u8) -> bool {
        let sender = self.sender.clone();
        let result = sender.try_send(trace_info);
        if let Err(e) = result {
            match e {
                TrySendError::Full(ctx) => {
                    if try_times > 3 {
                        error!(
                            "skywalking channel is full!traceId:{:?} msg log to disk.",
                            ctx.trace_id()
                        );
                        error!("tracing_ctx_info:{:?}", ctx);
                    }
                }
                TrySendError::Closed(ctx) => {
                    error!(
                        "skywalking channel is disconnected! searious error !traceId:{:?} msg log to disk.",
                        ctx.trace_id()
                    );
                    error!("tracing_ctx_info:{:?}", ctx);
                }
            }
            return false;
        }
        true
    }
}

impl Reporter {
    fn gen_segment(ctx: &Box<TracingContext>, config: &Config) -> SegmentObject {
        let mut spans: Vec<SpanObject> = Vec::new();
        for s in &ctx.finished_spans {
            let span_type = {
                if s.is_exit() {
                    1 //exit
                } else if s.is_entry() {
                    0 //entry
                } else {
                    2 //local
                }
            };
            let mut refs_vec = Vec::new();
            let key_tags = {
                let mut v = Vec::new();
                for t in s.tags() {
                    let key = t.key();
                    let value = t.value();
                    if key == "sw8-parent" {
                        if let Some(segment_ref) = SegmentReference::from_text(&value){
                            refs_vec.push(segment_ref)
                        }
                    }
                    v.push(KeyStringValuePair {
                        key: key,
                        value: value,
                    });
                }
                v
            };
            let span = SpanObject {
                span_id: s.span_id(),
                parent_span_id: s.parent_span_id(),
                start_time: s.time_info().0 as i64,
                end_time: s.time_info().1 as i64,
                refs: refs_vec,
                operation_name: s.operation_name().to_owned(),
                peer: s.peer_info().to_owned(),
                span_type: span_type,
                span_layer: s.span_layer().val(),
                component_id: 94,
                is_error: s.is_error(),
                tags: key_tags,
                logs: Vec::new(),
                skip_analysis: false,
            };
            spans.push(span);
        }

        let segment = SegmentObject {
            trace_id: ctx.trace_id().to_string(),
            trace_segment_id: ctx.segment_id().to_string(),
            spans,
            service: config.service_name.to_owned(),
            service_instance: config.service_instance.to_owned(),
            is_size_limited: false,
        };
        segment
    }

    pub fn new() -> Self {
        // skw????????????????????????
        const SKW_CHAN_SIZE: usize = 50000;
        let (sender, mut receiver) = mpsc::channel::<Box<TracingContext>>(SKW_CHAN_SIZE);

        let config = Config::new();
        GLOBAL_HANDLE.spawn(async move {
            let config = config.clone();
            let mut client = Self::loop_retrive_client(&config).await;
            let mut last_flush_time = Instant::now();
            let mut last_reconnect_time = Instant::now();
            let mut vec: Vec<Option<Box<TracingContext>>> = Vec::new();
            // ??????????????????time bucket, ????????????flush??????
            loop {
                let now = Instant::now();
                // reconnect skywalking gRPC service, in order to achieve (fake) load balancing
                if now.duration_since(last_reconnect_time).as_secs() >= RECONNECT_SECES {
                    last_reconnect_time = now;
                    client = Self::loop_retrive_client(&config).await;
                }

                // ??????????????????????????????REPORT_BUCKET
                if let Ok(ctx) =
                    timeout(Duration::from_millis(REPORT_BUCKET as u64), receiver.recv()).await
                {
                    vec.push(ctx);
                }
                // ???????????????????????????0???????????????????????????????????????
                if vec.is_empty() {
                    debug!("skw queue is empty in {} miliseconds", REPORT_BUCKET);
                    continue;
                }
                // channel recv??????????????????grpc?????????????????????
                if last_flush_time.elapsed().as_millis() >= REPORT_BUCKET || vec.len() > 500 {
                    debug!("start to flush to grpc tunnel...");
                    // ????????????????????????
                    Self::retry_send_skw(&mut client, &config, vec).await;
                    last_flush_time = Instant::now();
                    vec = Vec::new();
                }
            }
        });
        Reporter {
            config: Config::new(),
            sender,
        }
    }

    // retry connect operation until get a correct trace segment client
    // it will retry until the end of the world
    pub async fn loop_retrive_client(config: &Config) -> TraceSegmentReportServiceClient<Channel> {
        loop {
            let client_res = Self::retrieve_client(config).await;
            if let Ok(client) = client_res {
                return client;
            }
            // warn!("????????????skywalking???...");
            sleep(Duration::from_secs(1)).await;
        }
    }

    pub async fn retry_send_skw(
        client: &mut TraceSegmentReportServiceClient<Channel>,
        config: &Config,
        v_ctx: Vec<Option<Box<TracingContext>>>,
    ) {
        // ???????????????????????????????????????????????????????????????
        const RETRY_NUM: u8 = 5;
        let mut origin_ctx = v_ctx;
        // ??????RETRY NUM??????????????????
        for i in 0..RETRY_NUM {
            let (status, return_ctx) =
                Self::send_to_grpc_trace_service(client, config, origin_ctx).await;
            // ctx???????????????????????????????????????????????????
            origin_ctx = return_ctx;
            if status {
                if i > 0 {
                    warn!("retry success, current retry num is:{}", i);
                }
                // success! exit
                return;
            } else if i == RETRY_NUM - 1 {
                error!("skywalking trace collect method has met serious exception.segments backup to disk.{:?}", 
                         Self::transform_segments(config, &origin_ctx));
                // ???????????????????????????????????????
                return;
            } else {
                // ?????????????????????????????????????????????????????????
                // ??????500ms?????????????????????
                warn!("Request segments to grpc failed, enter into retry logic!");
                sleep(Duration::from_millis(500)).await;
            }
        }
    }

    // ????????????????????????????????????3????????????????????????????????????????????????
    // grpc?????????????????????????????????????????????????????????????????????????????????????????????????????????????????????
    pub async fn send_to_grpc_trace_service(
        client: &mut TraceSegmentReportServiceClient<Channel>,
        config: &Config,
        v_ctx: Vec<Option<Box<TracingContext>>>,
    ) -> (bool, Vec<Option<Box<TracingContext>>>) {
        let segments = Self::transform_segments(config, &v_ctx);
        let request = Request::new(stream::iter(segments));
        if let Err(e) = client.collect(request).await {
            match e.code() {
                // ???????????????????????????????????????:
                // 1.DeadlineExceeded: ?????????????????????
                // 2.ResourceExhausted: ?????????????????????
                // 3.Unavailable: ????????????????????????
                Code::DeadlineExceeded | Code::ResourceExhausted | Code::Unavailable => {
                    warn!("Request grpc error!{:?}", e);
                    return (false, v_ctx);
                }
                _ => {
                    // ????????????????????????,??????????????????skywalking??????
                    error!("grpc unknown exception occurred, error:{:?}", e);
                    let new_client = Self::loop_retrive_client(config).await;
                    *client = new_client;
                    return (false, v_ctx);
                }
            }
        }
        return (true, v_ctx);
    }

    // segments type transform from library format to grpc format
    pub fn transform_segments(
        config: &Config,
        ctx: &Vec<Option<Box<TracingContext>>>,
    ) -> Vec<SegmentObject> {
        let mut segments_buffer = Vec::new();
        for v in ctx {
            if let Some(ctx) = v {
                let segment = Self::gen_segment(ctx, &config);
                segments_buffer.push(segment);
            }
        }
        segments_buffer
    }

    pub async fn retrieve_client(
        config: &Config,
    ) -> Result<TraceSegmentReportServiceClient<Channel>> {
        let tracing_client = TraceSegmentReportServiceClient::connect(format!(
            "http://{}:{}",
            config
                .collector_host
                .clone()
                .unwrap_or("127.0.0.1".to_owned()),
            config.collector_port.clone().unwrap_or("11800".to_owned())
        ))
        .await?;
        Ok(tracing_client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skywalking::agent::ContextManager;
    use crate::skywalking::core::SpanLayer;
    use env_logger;
    use env_logger::Env;
    use std::{thread, time};

    #[test]
    fn test_send_segs() {
        // use std::time::{SystemTime, UNIX_EPOCH};
        // let start = SystemTime::now();
        // let since_the_epoch = start
        //     .duration_since(UNIX_EPOCH)
        //     .expect("Time went backwards");
        // println!("{:?}", since_the_epoch);
        env_logger::from_env(Env::default().default_filter_or("info")).init();
        let mut run = Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("create tokio runtime fail");
        run.block_on(async move {
            for _ in 0..2000 {
                for _ in 0..10 {
                    let _ = ContextManager::async_enter(async { execute().await }).await;
                }
                delay_for(Duration::from_secs(1)).await;
            }
        });
        let ten_millis = time::Duration::from_secs(1000);
        thread::sleep(ten_millis);
    }

    #[test]
    fn test_not_set_async_env() {
        let mut run = Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("create tokio runtime fail");
        run.block_on(async move { async { execute().await }.await });
    }

    // ????????????????????????
    async fn execute() {
        let entry_span = ContextManager::tracing_entry("/xxx/xxx", SpanLayer::HTTP, None);
        let exit_span =
            ContextManager::tracing_exit("rpc.service:1.0", "127.0.0.1", SpanLayer::Rpc, None);
        ContextManager::finish_span(exit_span);
        ContextManager::finish_span(entry_span);
    }
}
