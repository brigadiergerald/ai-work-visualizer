//! Fixed Buffer documents for manual Threads scheduling. No generic GraphQL surface.
use serde_json::{json,Value};
pub const TARGET:&str="query ManualTarget($channelId: ChannelId!) { account { id } channel(input: {id: $channelId}) { id organizationId name service isDisconnected isLocked isQueuePaused allowedActions timezone } }";
pub const LIST:&str="query ManualPosts($organizationId: OrganizationId!, $channelId: ChannelId!, $after: String) { posts(first: 100, after: $after, input: {organizationId: $organizationId, filter: {channelIds: [$channelId]}}) { edges { node { id channelId text dueAt status externalLink } } pageInfo { hasNextPage endCursor } } }";
pub const CREATE:&str="mutation ManualSchedule($input: CreatePostInput!) { createPost(input: $input) { __typename ... on PostActionSuccess { post { id channelId text dueAt status externalLink } } ... on MutationError { message } } }";
pub const POST:&str="query ManualPost($id: PostId!) { post(input: {id: $id}) { id channelId text dueAt status externalLink } }";
pub const CREATE_NOW:&str="mutation ManualPublishNow($input: CreatePostInput!) { createPost(input: $input) { __typename ... on PostActionSuccess { post { id channelId text dueAt status externalLink shareMode createdAt } } ... on MutationError { message } } }";
pub const LIST_NOW:&str="query ManualImmediatePosts($organizationId: OrganizationId!, $channelId: ChannelId!, $after: String) { posts(first: 100, after: $after, input: {organizationId: $organizationId, filter: {channelIds: [$channelId]}}) { edges { node { id channelId text dueAt status externalLink shareMode createdAt } } pageInfo { hasNextPage endCursor } } }";
pub const POST_NOW:&str="query ManualImmediatePost($id: PostId!) { post(input: {id: $id}) { id channelId text dueAt status externalLink shareMode createdAt } }";
fn id(v:&Value)->bool{v.as_str().is_some_and(|s|!s.is_empty()&&s.len()<=128&&s.bytes().all(|b|b.is_ascii_alphanumeric()||b"-_".contains(&b)))}
pub fn body(query:&str,variables:Value)->Vec<u8>{serde_json::to_vec(&json!({"query":query,"variables":variables})).unwrap()}
pub fn approved_body(raw:&[u8])->bool{
 let Ok(v)=serde_json::from_slice::<Value>(raw)else{return false};let Some(o)=v.as_object()else{return false};if o.len()!=2{return false}
 let vars=&v["variables"];let Some(obj)=vars.as_object()else{return false};
 match v["query"].as_str(){
  Some(TARGET)=>obj.len()==1&&id(&vars["channelId"]),
  Some(POST)|Some(POST_NOW)=>obj.len()==1&&id(&vars["id"]),
  Some(LIST)|Some(LIST_NOW)=>obj.len()==3&&id(&vars["organizationId"])&&id(&vars["channelId"])&&(vars["after"].is_null()||vars["after"].as_str().is_some_and(|s|s.len()<=1024)),
  Some(CREATE)=>{let i=&vars["input"];obj.len()==1&&i.as_object().is_some_and(|m|m.len()==5)&&id(&i["channelId"])&&i["mode"]=="customScheduled"&&i["schedulingType"]=="automatic"&&i["text"].as_str().is_some_and(|t|!t.trim().is_empty()&&t.chars().count()<=500)&&i["dueAt"].as_str().is_some_and(|s|chrono::DateTime::parse_from_rfc3339(s).is_ok())},
  Some(CREATE_NOW)=>{let i=&vars["input"];obj.len()==1&&i.as_object().is_some_and(|m|m.len()==4)&&id(&i["channelId"])&&i["mode"]=="shareNow"&&i["schedulingType"]=="automatic"&&i["text"].as_str().is_some_and(|t|!t.trim().is_empty()&&t.chars().count()<=500)},
  _=>false
 }
}
pub trait Transport{fn exchange(&mut self,body:&[u8])->Result<crate::provider_http::Reply,crate::provider_http::Error>;}
pub struct Buffer(pub crate::provider_credentials::Secret);
impl Transport for Buffer{fn exchange(&mut self,body:&[u8])->Result<crate::provider_http::Reply,crate::provider_http::Error>{if !approved_body(body){return Err(crate::provider_http::Error::Transport)}crate::provider_http::request(&self.0,body)}}
