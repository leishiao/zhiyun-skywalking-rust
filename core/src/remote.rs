#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeyStringValuePair {
    #[prost(string, tag = "1")]
    pub key: std::string::String,
    #[prost(string, tag = "2")]
    pub value: std::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Cpu {
    #[prost(double, tag = "2")]
    pub usage_percent: f64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Commands {
    #[prost(message, repeated, tag = "1")]
    pub commands: ::std::vec::Vec<Command>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Command {
    #[prost(string, tag = "1")]
    pub command: std::string::String,
    #[prost(message, repeated, tag = "2")]
    pub args: ::std::vec::Vec<KeyStringValuePair>,
}
/// In most cases, detect point should be `server` or `client`.
/// Even in service mesh, this means `server`/`client` side sidecar
/// `proxy` is reserved only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum DetectPoint {
    Client = 0,
    Server = 1,
    Proxy = 2,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SegmentObject {
    #[prost(string, tag = "1")]
    pub trace_id: std::string::String,
    #[prost(string, tag = "2")]
    pub trace_segment_id: std::string::String,
    #[prost(message, repeated, tag = "3")]
    pub spans: ::std::vec::Vec<SpanObject>,
    #[prost(string, tag = "4")]
    pub service: std::string::String,
    #[prost(string, tag = "5")]
    pub service_instance: std::string::String,
    #[prost(bool, tag = "6")]
    pub is_size_limited: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SegmentReference {
    #[prost(enumeration = "RefType", tag = "1")]
    pub ref_type: i32,
    #[prost(string, tag = "2")]
    pub trace_id: std::string::String,
    #[prost(string, tag = "3")]
    pub parent_trace_segment_id: std::string::String,
    #[prost(int32, tag = "4")]
    pub parent_span_id: i32,
    #[prost(string, tag = "5")]
    pub parent_service: std::string::String,
    #[prost(string, tag = "6")]
    pub parent_service_instance: std::string::String,
    #[prost(string, tag = "7")]
    pub parent_endpoint: std::string::String,
    #[prost(string, tag = "8")]
    pub network_address_used_at_peer: std::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpanObject {
    #[prost(int32, tag = "1")]
    pub span_id: i32,
    #[prost(int32, tag = "2")]
    pub parent_span_id: i32,
    #[prost(int64, tag = "3")]
    pub start_time: i64,
    #[prost(int64, tag = "4")]
    pub end_time: i64,
    #[prost(message, repeated, tag = "5")]
    pub refs: ::std::vec::Vec<SegmentReference>,
    #[prost(string, tag = "6")]
    pub operation_name: std::string::String,
    #[prost(string, tag = "7")]
    pub peer: std::string::String,
    #[prost(enumeration = "SpanType", tag = "8")]
    pub span_type: i32,
    #[prost(enumeration = "SpanLayer", tag = "9")]
    pub span_layer: i32,
    #[prost(int32, tag = "10")]
    pub component_id: i32,
    #[prost(bool, tag = "11")]
    pub is_error: bool,
    #[prost(message, repeated, tag = "12")]
    pub tags: ::std::vec::Vec<KeyStringValuePair>,
    #[prost(message, repeated, tag = "13")]
    pub logs: ::std::vec::Vec<Log>,
    #[prost(bool, tag = "14")]
    pub skip_analysis: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Log {
    #[prost(int64, tag = "1")]
    pub time: i64,
    #[prost(message, repeated, tag = "2")]
    pub data: ::std::vec::Vec<KeyStringValuePair>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Id {
    #[prost(string, repeated, tag = "1")]
    pub id: ::std::vec::Vec<std::string::String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SpanType {
    Entry = 0,
    Exit = 1,
    Local = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RefType {
    CrossProcess = 0,
    CrossThread = 1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SpanLayer {
    Unknown = 0,
    Database = 1,
    RpcFramework = 2,
    Http = 3,
    Mq = 4,
    Cache = 5,
}
#[doc = r" Generated client implementations."]
pub mod trace_segment_report_service_client {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    pub struct TraceSegmentReportServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl TraceSegmentReportServiceClient<tonic::transport::Channel> {
        #[doc = r" Attempt to create a new client by connecting to a given endpoint."]
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> TraceSegmentReportServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::ResponseBody: Body + HttpBody + Send + 'static,
        T::Error: Into<StdError>,
        <T::ResponseBody as HttpBody>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = tonic::client::Grpc::with_interceptor(inner, interceptor);
            Self { inner }
        }
        pub async fn collect(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::SegmentObject>,
        ) -> Result<tonic::Response<super::Commands>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/TraceSegmentReportService/collect");
            self.inner
                .client_streaming(request.into_streaming_request(), path, codec)
                .await
        }
    }
    impl<T: Clone> Clone for TraceSegmentReportServiceClient<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }
    impl<T> std::fmt::Debug for TraceSegmentReportServiceClient<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "TraceSegmentReportServiceClient {{ ... }}")
        }
    }
}
#[doc = r" Generated server implementations."]
pub mod trace_segment_report_service_server {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    #[doc = "Generated trait containing gRPC methods that should be implemented for use with TraceSegmentReportServiceServer."]
    #[async_trait]
    pub trait TraceSegmentReportService: Send + Sync + 'static {
        async fn collect(
            &self,
            request: tonic::Request<tonic::Streaming<super::SegmentObject>>,
        ) -> Result<tonic::Response<super::Commands>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct TraceSegmentReportServiceServer<T: TraceSegmentReportService> {
        inner: _Inner<T>,
    }
    struct _Inner<T>(Arc<T>, Option<tonic::Interceptor>);
    impl<T: TraceSegmentReportService> TraceSegmentReportServiceServer<T> {
        pub fn new(inner: T) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner, None);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner, Some(interceptor.into()));
            Self { inner }
        }
    }
    impl<T, B> Service<http::Request<B>> for TraceSegmentReportServiceServer<T>
    where
        T: TraceSegmentReportService,
        B: HttpBody + Send + Sync + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = Never;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/TraceSegmentReportService/collect" => {
                    #[allow(non_camel_case_types)]
                    struct collectSvc<T: TraceSegmentReportService>(pub Arc<T>);
                    impl<T: TraceSegmentReportService>
                        tonic::server::ClientStreamingService<super::SegmentObject>
                        for collectSvc<T>
                    {
                        type Response = super::Commands;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<tonic::Streaming<super::SegmentObject>>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).collect(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1;
                        let inner = inner.0;
                        let method = collectSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.client_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => Box::pin(async move {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", "12")
                        .body(tonic::body::BoxBody::empty())
                        .unwrap())
                }),
            }
        }
    }
    impl<T: TraceSegmentReportService> Clone for TraceSegmentReportServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self { inner }
        }
    }
    impl<T: TraceSegmentReportService> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone(), self.1.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: TraceSegmentReportService> tonic::transport::NamedService
        for TraceSegmentReportServiceServer<T>
    {
        const NAME: &'static str = "TraceSegmentReportService";
    }
}
