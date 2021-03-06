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

use std::time::SystemTime;

use crate::skywalking::core::log::LogEvent;
use crate::skywalking::core::segment_ref::SegmentRef;
use crate::skywalking::core::Tag;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;

/// Span is one of the tracing concept, representing a time duration.
///Span is an important and common concept in distributed tracing system. Learn Span from Google Dapper Paper.
/// For better performance, we extend the span into 3 kinds.
///
/// 1. EntrySpan EntrySpan represents a service provider, also the endpoint of server side. As an APM system, we are targeting the application servers. So almost all the services and MQ-consumer are EntrySpan(s).
/// 2. LocalSpan LocalSpan represents a normal Java method, which does not relate to remote service, neither a MQ producer/consumer nor a service(e.g. HTTP service) provider/consumer.
/// 3. ExitSpan ExitSpan represents a client of service or MQ-producer, as named as LeafSpan at early age of SkyWalking. e.g. accessing DB by JDBC, reading Redis/Memcached are cataloged an ExitSpan.
pub trait Span: SpanClone {
    /// Start the span with the current system time
    fn start(&mut self);
    /// Start the span by using given time point.
    fn start_with_timestamp(&mut self, timestamp: SystemTime);
    /// Add a new tag to the span
    fn tag(&mut self, tag: Tag);
    /// Add a log event to the span
    fn log(&mut self, log: LogEvent);
    /// Indicate error occurred during the span execution.
    fn error_occurred(&mut self);
    /// Set the component id represented by this span.
    /// Component id is pre-definition in the SkyWalking OAP backend component-libraries.yml file.
    /// Read [Component library settings](https://github.com/apache/skywalking/blob/master/docs/en/guides/Component-library-settings.md) documentation for more details
    fn set_component_id(&mut self, component_id: i32);
    /// End the span with the current system time.
    /// End just means sealing the end time, still need to call Context::finish_span to officially finish span and archive it for further reporting.
    fn end(&mut self);
    /// End the span by using given time point.
    /// End just means sealing the end time, still need to call Context::finish_span to officially finish span and archive it for further reporting.
    fn end_with_timestamp(&mut self, timestamp: SystemTime);

    /// All following are status reading methods.

    /// Return true if the span has been set end time
    fn is_ended(&self) -> bool;
    /// Return true if the span is an entry span
    fn is_entry(&self) -> bool;
    /// Return true if the span is an exit span
    fn is_exit(&self) -> bool;
    /// return true is the span is error or
    fn is_error(&self) -> bool;
    /// Return span id.
    fn span_id(&self) -> i32;
    /// Return parent span id
    fn parent_span_id(&self) -> i32;
    /// Return the replicated existing tags.
    fn tags(&self) -> Vec<Tag>;
    /// Return operation name
    fn operation_name(&self) -> &str;
    /// Return time info, tuple: (start_time, end_time)
    fn time_info(&self) -> (u64, u64);
    /// merge arg span tags into to current span, peer info
    fn merge_span(&mut self, span: Box<dyn Span + Send>);
    /// set peer info
    fn set_peer(&mut self, peer: &str);
    /// get peer info
    fn peer_info(&self) -> &str;
    /// return span layer
    fn span_layer(&self) -> SpanLayer;
}

pub trait SpanClone {
    fn clone_box(&self) -> Box<dyn Span + Send>;
}

impl<T> SpanClone for T
where
    T: 'static + Span + Send + Clone,
{
    fn clone_box(&self) -> Box<dyn Span + Send> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Span + Send> {
    fn clone(&self) -> Box<dyn Span + Send> {
        self.clone_box()
    }
}

impl Debug for Box<dyn Span + Send> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let _ = f.write_str(&format!(
            "operation_name:{},time:{:?}, tags:{:?}, is_ended:{}, is_exit:{}, is_entry:{}",
            self.operation_name(),
            self.time_info(),
            self.tags(),
            self.is_ended(),
            self.is_exit(),
            self.is_entry()
        ));
        Ok(())
    }
}

#[derive(Clone)]
pub struct TracingSpan {
    /// The operation name represents the logic process of this span
    operation_name: String,
    span_id: i32,
    parent_span_id: i32,
    /// The timestamp of the span start time
    start_time: u64,
    /// The timestamp of the span end time
    end_time: u64,
    /// As an entry span
    is_entry: bool,
    /// As an exit span
    is_exit: bool,
    /// The peer network address when as an RPC related span.
    /// Typically used in exit span, representing the target server address.
    peer: Option<String>,
    /// Tag this span in error status.
    error_occurred: bool,
    /// Component id is defined in the main repo to represent the library kind.
    component_id: Option<i32>,
    tags: Vec<Tag>,
    logs: Vec<LogEvent>,
    refs: Vec<SegmentRef>,
    span_layer: SpanLayer,
}

#[derive(Clone)]
pub enum SpanLayer {
    DB,
    Rpc,
    HTTP,
    MQ,
    CACHE,
}

impl SpanLayer {
    pub fn val(&self) -> i32 {
        match self {
            SpanLayer::DB => 1,
            SpanLayer::Rpc => 2,
            SpanLayer::HTTP => 3,
            SpanLayer::MQ => 4,
            SpanLayer::CACHE => 5,
        }
    }
}

/// Tracing Span is only created inside TracingContext.
impl TracingSpan {
    /// Create a new entry span
    pub fn new_entry_span(
        operation_name: &str,
        span_id: i32,
        parent_span_id: i32,
        layer: SpanLayer,
    ) -> TracingSpan {
        let mut span = TracingSpan::_new(operation_name, span_id, parent_span_id, layer);
        span.is_entry = true;
        span
    }

    /// Create a new exit span
    pub fn new_exit_span(
        operation_name: &str,
        span_id: i32,
        parent_span_id: i32,
        peer: &str,
        layer: SpanLayer,
    ) -> TracingSpan {
        let mut span = TracingSpan::_new(operation_name, span_id, parent_span_id, layer);
        span.is_exit = true;
        span.peer = Some(String::from(peer));
        span
    }

    /// Create a new local span
    pub fn new_local_span(
        operation_name: &str,
        span_id: i32,
        parent_span_id: i32,
        layer: SpanLayer,
    ) -> TracingSpan {
        let span = TracingSpan::_new(operation_name, span_id, parent_span_id, layer);
        span
    }

    /// Create a span
    fn _new(
        operation_name: &str,
        span_id: i32,
        parent_span_id: i32,
        span_layer: SpanLayer,
    ) -> Self {
        TracingSpan {
            operation_name: String::from(operation_name),
            span_id,
            parent_span_id,
            start_time: 0,
            end_time: 0,
            is_entry: false,
            is_exit: false,
            peer: None,
            error_occurred: false,
            component_id: None,
            tags: Vec::new(),
            logs: Vec::new(),
            refs: Vec::new(),
            span_layer,
        }
    }

    pub fn _add_ref(&mut self, reference: SegmentRef) {
        self.refs.push(reference);
    }
}

impl Span for TracingSpan {
    fn start(&mut self) {
        self.start_time = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => n.as_millis(),
            Err(_) => self.start_time as u128,
        } as u64;
    }

    fn start_with_timestamp(&mut self, timestamp: SystemTime) {
        self.start_time = match timestamp.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => n.as_millis(),
            Err(_) => self.start_time as u128,
        } as u64;
    }

    fn tag(&mut self, tag: Tag) {
        self.tags.push(tag);
    }

    fn log(&mut self, log: LogEvent) {
        self.logs.push(log);
    }

    fn error_occurred(&mut self) {
        self.error_occurred = true;
    }

    fn set_component_id(&mut self, component_id: i32) {
        self.component_id = Some(component_id);
    }

    fn end(&mut self) {
        self.end_time = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => n.as_millis(),
            Err(_) => self.start_time as u128,
        } as u64;
    }

    fn end_with_timestamp(&mut self, timestamp: SystemTime) {
        self.end_time = match timestamp.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => n.as_millis(),
            Err(_) => self.start_time as u128,
        } as u64;
    }

    fn is_ended(&self) -> bool {
        self.end_time != 0
    }

    fn is_entry(&self) -> bool {
        self.is_entry
    }

    fn is_exit(&self) -> bool {
        self.is_exit
    }

    fn is_error(&self) -> bool {
        self.error_occurred
    }

    fn span_id(&self) -> i32 {
        self.span_id
    }

    fn parent_span_id(&self) -> i32 {
        self.parent_span_id
    }

    fn tags(&self) -> Vec<Tag> {
        let mut tags = Vec::new();
        for t in &self.tags {
            tags.push(t.clone());
        }
        tags
    }

    fn operation_name(&self) -> &str {
        self.operation_name.as_str()
    }

    fn time_info(&self) -> (u64, u64) {
        (self.start_time, self.end_time)
    }

    fn merge_span(&mut self, span: Box<dyn Span + Send>) {
        let tags = span.tags();
        for tag in tags {
            self.tag(tag);
        }
        if span.is_error() {
            self.error_occurred();
        }
    }

    fn peer_info(&self) -> &str {
        if let Some(s) = &self.peer {
            return s;
        }
        return "";
    }

    fn set_peer(&mut self, peer: &str) {
        self.peer = Some(peer.to_owned());
    }

    fn span_layer(&self) -> SpanLayer {
        self.span_layer.clone()
    }
}

#[cfg(test)]
mod span_tests {
    use std::time::SystemTime;

    use crate::skywalking::core::log::{EventField, LogEvent};
    use crate::skywalking::core::span::*;
    use crate::skywalking::core::Tag;

    #[test]
    fn test_span_new() {
        let mut span = TracingSpan::_new("op1", 0, -1, SpanLayer::Rpc);
        assert_eq!(span.parent_span_id, -1);
        assert_eq!(span.span_id, 0);
        assert_eq!(span.start_time, 0);
        span.start();
        assert_ne!(span.start_time, 0);

        let mut span2 = TracingSpan::_new("op2", 1, 0, SpanLayer::Rpc);
        assert_eq!("op2", span2.operation_name);
        assert_eq!(span2.parent_span_id, 0);
        assert_eq!(span2.span_id, 1);
        span2.start_with_timestamp(SystemTime::now());
        assert_ne!(span2.start_time, 0);
    }

    #[test]
    fn test_new_entry_span() {
        let span = TracingSpan::new_entry_span("op1", 0, 1, SpanLayer::Rpc);
        assert_eq!(span.is_entry(), true)
    }

    #[test]
    fn test_span_with_tags() {
        let mut span = TracingSpan::new_entry_span("op1", 0, 1, SpanLayer::Rpc);
        span.tag(Tag::new(String::from("tag1"), String::from("value1")));
        span.tag(Tag::new(String::from("tag2"), String::from("value2")));

        let tags = span.tags();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags.get(0).unwrap().key(), "tag1")
    }

    #[test]
    fn test_span_with_logs() {
        let mut span = TracingSpan::_new("op1", 0, -1, SpanLayer::Rpc);

        span.log(LogEvent::new(
            123,
            Box::new([
                { EventField::new(String::from("event1"), String::from("event description")) },
                { EventField::new(String::from("event2"), String::from("event description")) },
            ]),
        ));

        assert_eq!(span.logs.len(), 1);
    }

    #[test]
    fn test_box_span_clone() {
        let span = TracingSpan::new_entry_span("op1", 0, 1, SpanLayer::Rpc);
        let b: Box<dyn Span + Send> = Box::new(span);
        let _ = b.clone();
    }
}
