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

use lazy_static::lazy_static;
use std::borrow::{Borrow, BorrowMut};
use std::cell::RefCell;
use tokio::task_local;

use crate::skywalking::agent::reporter::Reporter;
use crate::skywalking::core::{
    Context, ContextListener, Extractable, Injectable, Span, TracingContext,
};
use std::future::Future;

task_local! {
    static CTX: RefCell<Box<CurrentTracingContext>>;
}

lazy_static! {
    static ref SKYWALKING_REPORTER: Reporter = { Reporter::new() };
}

pub struct ContextManager {}

impl ContextManager {
    // create a span and return a span, the span is start automatically
    // you need manually start this span
    pub fn tracing_entry(
        operation_name: &str,
        extractor: Option<&dyn Extractable>,
    ) -> Option<Box<dyn Span + Send>> {
        return CTX.with(|context| {
            let mut span;
            {
                // Borrow mut ref has to end in this specific scope, as the context is nested used in f<F>
                let mut mut_context = context.borrow_mut();
                let parent_span_id = mut_context.parent_span_id();
                span = mut_context.create_entry_span(operation_name, parent_span_id, extractor);
            }
            if let Some(s) = span.as_mut() {
                s.start();
            }
            span
        });
    }

    // end a span and finish a span
    pub fn finish_span(span: Option<Box<dyn Span + Send>>) {
        CTX.with(|context| {
            if let Some(mut span) = span {
                span.end();
                let is_first_span = span.span_id() == 0;
                let mut mut_context = context.borrow_mut();
                mut_context.finish_span(span);

                if is_first_span {
                    mut_context.finish();
                }
            }
        });
    }

    // create exit span
    // span is automatically started
    pub fn tracing_exit<F>(
        operation_name: &str,
        peer: &str,
        injector: Option<&dyn Injectable>,
    ) -> Option<Box<dyn Span + Send>> {
        return CTX.with(|context| {
            let mut span;
            {
                // Borrow mut ref has to end in this specific scope, as the context is nested used in f<F>
                let mut mut_context = context.borrow_mut();
                let parent_span_id = mut_context.parent_span_id();
                span = mut_context.create_exit_span(operation_name, parent_span_id, peer, injector);
            }
            if let Some(s) = span.as_mut() {
                s.start();
            }
            span
        });
    }

    // create local span
    // span is automatically started
    pub fn tracing_local<F>(operation_name: &str) -> Option<Box<dyn Span + Send>> {
        return CTX.with(|context| {
            let mut span;
            {
                // Borrow mut ref has to end in this specific scope, as the context is nested used in f<F>
                let mut mut_context = context.borrow_mut();
                let parent_span_id = mut_context.parent_span_id();
                span = mut_context.create_local(operation_name, parent_span_id);
            }
            if let Some(s) = span.as_mut() {
                s.start();
            }
            span
        });
    }

    // when first time enter into skywalking async enviroment, call this method
    pub fn async_enter<T, F>(f: F) -> impl Future<Output = T>
    where
        F: Future<Output = T>,
    {
        let tracing_ctx = CurrentTracingContext::new();
        CTX.scope(RefCell::new(Box::new(tracing_ctx)), async { f.await })
    }
}

pub struct CurrentTracingContext {
    option: Option<Box<WorkingContext>>,
}

struct WorkingContext {
    context: Box<TracingContext>,
    span_stack: Vec<i32>,
}

impl CurrentTracingContext {
    /// Create the tracing context in the thread local at the first time.
    pub fn new() -> Self {
        CurrentTracingContext {
            option: match TracingContext::new(SKYWALKING_REPORTER.service_instance_id()) {
                Some(tx) => Some(Box::new(WorkingContext {
                    context: Box::new(tx),
                    span_stack: Vec::new(),
                })),
                None => None,
            },
        }
    }

    /// Delegate to the tracing core entry span creation method, if current context is valid.
    fn create_entry_span(
        &mut self,
        operation_name: &str,
        parent_span_id: Option<i32>,
        extractor: Option<&dyn Extractable>,
    ) -> Option<Box<dyn Span + Send>> {
        match self.option.borrow_mut() {
            None => None,
            Some(wx) => {
                let span = wx
                    .context
                    .create_entry_span(operation_name, parent_span_id, extractor);
                wx.span_stack.push(span.span_id());
                Some(span)
            }
        }
    }

    /// Delegate to the tracing core exit span creation method, if current context is valid.
    fn create_exit_span(
        &mut self,
        operation_name: &str,
        parent_span_id: Option<i32>,
        peer: &str,
        injector: Option<&dyn Injectable>,
    ) -> Option<Box<dyn Span + Send>> {
        match self.option.borrow_mut() {
            None => None,
            Some(wx) => {
                let span =
                    wx.context
                        .create_exit_span(operation_name, parent_span_id, peer, injector);
                wx.span_stack.push(span.span_id());
                Some(span)
            }
        }
    }

    /// Delegate to the tracing core local span creation method, if current context is valid.
    fn create_local(
        &mut self,
        operation_name: &str,
        parent_span_id: Option<i32>,
    ) -> Option<Box<dyn Span + Send>> {
        match self.option.borrow_mut() {
            None => None,
            Some(wx) => {
                let span = wx.context.create_local_span(operation_name, parent_span_id);
                wx.span_stack.push(span.span_id());
                Some(span)
            }
        }
    }

    /// Delegate to the tracing core span finish method, if current context is valid.
    fn finish_span(&mut self, span: Box<dyn Span + Send>) {
        match self.option.borrow_mut() {
            None => {}
            Some(wx) => {
                wx.context.finish_span(span);
                wx.span_stack.pop();
            }
        };
    }

    /// Fetch the parent span id, to be used in next new span.
    /// The span id(s) are saved in the span_stack by following as same the stack-style as span creation sequence.
    fn parent_span_id(&self) -> Option<i32> {
        match self.option.borrow() {
            None => None,
            Some(wx) => match wx.span_stack.last() {
                None => None,
                Some(span_id) => Some(span_id.clone()),
            },
        }
    }

    /// Finish the current tracing context, including
    /// 1. Clear up the context
    /// 2. Transfer the context to profobuf format and pass to reporter.
    fn finish(&mut self) {
        match self.option.borrow_mut() {
            None => {}
            Some(wx) => {
                let tracingContext = &wx.context;

                wx.span_stack.clear();

                // TODO: Transfer tracingContext to protobuf
            }
        }
        self.option = None;
    }
}

#[cfg(test)]
mod context_tests {
    use crate::skywalking::agent::context_manager::*;
    use crate::skywalking::core::{ContextListener, Tag, TracingContext};
    use env_logger::Env;
    use tokio::runtime::Runtime;

    #[test]
    fn test_context_manager() {
        // env_logger::from_env(Env::default().default_filter_or("info")).init();

        // let mut run = Runtime::new().unwrap();
        // run.block_on(async {
        //     ContextManager::async_enter(execute()).await;
        // });
    }

    // async fn execute() {
    //     ContextManager::tracing_entry("op1", None, |mut span| {
    //         span.tag(Tag::new(String::from("tag1"), String::from("value1")));
    //         for tag in &span.tags() {
    //             println!("key:{}, value:{}", tag.key(), tag.value());
    //         }
    //         println!("spanId:{}", span.span_id());
    //         ContextManager::tracing_exit("op2", "127.0.0.1:8080", None, |mut span| {
    //             for tag in &span.tags() {
    //                 println!("key:{}, value:{}", tag.key(), tag.value());
    //             }
    //             println!("spanId:{}", span.span_id());
    //             span.set_component_id(33);
    //         });

    //         ContextManager::tracing_local("op3", |mut span| {});
    //     });
    // }

    struct MockReporter {}

    impl ContextListener for MockReporter {
        fn service_instance_id(&self) -> Option<i32> {
            Some(1)
        }

        fn report_trace(&self, finished_context: Box<TracingContext>) {
            println!(
                "finally finished span is:{:?}",
                finished_context.finished_spans
            );
        }
    }
}
