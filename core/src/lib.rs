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

// 1.skywalking的proto协议中没有package的设置，但是tonic强制要求必须
// 在proto中定义package的信息;
// 2.Trace接口中的 collect方法和集合中的方法发生重合导致项目编译提示问题
// 综合上述两点：
// 直接采用生成的rust grp client代码嵌入到项目中的方式
//
// pub mod remote {
//     tonic::include_proto!("remote");
// }
mod remote;
pub mod skywalking;
