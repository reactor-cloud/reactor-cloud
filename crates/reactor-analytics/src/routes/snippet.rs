//! GET /analytics/v1/snippet.js - Serve the analytics UMD bundle.
//!
//! This endpoint serves a lightweight analytics script that can be injected
//! into HTML pages via `<script>` tag.

use axum::{
    extract::Query,
    http::{header, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;

/// Query parameters for snippet.js.
#[derive(Debug, Deserialize)]
pub struct SnippetParams {
    /// Project key to embed in the snippet.
    pub key: String,
    /// API endpoint (optional, defaults to same origin).
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Auto-capture pageviews.
    #[serde(default = "default_true")]
    pub pageview: bool,
    /// Auto-capture errors.
    #[serde(default)]
    pub errors: bool,
    /// Auto-capture clicks.
    #[serde(default)]
    pub capture: bool,
}

fn default_true() -> bool {
    true
}

/// Serve the analytics snippet.
pub async fn snippet(Query(params): Query<SnippetParams>) -> impl IntoResponse {
    let endpoint = params.endpoint.unwrap_or_else(|| "/analytics/v1".to_string());
    
    let script = generate_snippet(&params.key, &endpoint, params.pageview, params.errors, params.capture);
    
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=86400, stale-while-revalidate=604800"),
        ],
        script,
    )
}

/// Generate the analytics snippet JavaScript.
fn generate_snippet(
    project_key: &str,
    endpoint: &str,
    auto_pageview: bool,
    auto_errors: bool,
    auto_capture: bool,
) -> String {
    let base = SNIPPET_TEMPLATE
        .replace("{{ENDPOINT}}", endpoint)
        .replace("{{KEY}}", project_key);

    let pageview_setup = if auto_pageview {
        PAGEVIEW_SETUP
    } else {
        ""
    };

    let error_setup = if auto_errors {
        ERROR_SETUP
    } else {
        ""
    };

    let capture_setup = if auto_capture {
        CAPTURE_SETUP
    } else {
        ""
    };

    base.replace("{{PAGEVIEW_SETUP}}", pageview_setup)
        .replace("{{ERROR_SETUP}}", error_setup)
        .replace("{{CAPTURE_SETUP}}", capture_setup)
}

const SNIPPET_TEMPLATE: &str = r#"!function(){
"use strict";
var SDK_NAME="@reactor/analytics",SDK_VERSION="0.1.0",BATCH_SIZE=20,FLUSH_INTERVAL=5000,STORAGE_KEY="reactor_anon_id",ERROR_DEDUPE_MS=5000;
var ENDPOINT="{{ENDPOINT}}",KEY="{{KEY}}";

function generateId(){return crypto&&crypto.randomUUID?crypto.randomUUID():"xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g,function(c){var r=16*Math.random()|0;return("x"===c?r:3&r|8).toString(16)})};

function getAnonymousId(){if("undefined"!=typeof localStorage){var stored=localStorage.getItem(STORAGE_KEY);if(stored)return stored;var newId=generateId();return localStorage.setItem(STORAGE_KEY,newId),newId}return generateId()};

function getContext(){var ctx={library:{name:SDK_NAME,version:SDK_VERSION}};return"undefined"!=typeof navigator&&(ctx.userAgent=navigator.userAgent,ctx.locale=navigator.language),"undefined"!=typeof Intl&&(ctx.timezone=Intl.DateTimeFormat().resolvedOptions().timeZone),"undefined"!=typeof screen&&(ctx.screen={width:screen.width,height:screen.height}),"undefined"!=typeof window&&"undefined"!=typeof document&&(ctx.path=window.location.pathname,ctx.url=window.location.href,ctx.referrer=document.referrer||void 0,ctx.title=document.title||void 0),ctx};

var queue=[],flushTimer=null,optedOut=!1,anonId=getAnonymousId(),sessionId=generateId(),userId=void 0,errorFingerprints={};

function track(event,props){if(!optedOut){var e={event:event,properties:props||{},timestamp:(new Date).toISOString(),anonymousId:anonId,userId:userId,sessionId:sessionId,context:getContext()};queue.push(e),queue.length>=BATCH_SIZE&&flush()}};

function page(name,props){var ctx=getContext();track("$pageview",Object.assign({name:name||ctx.title,path:ctx.path,url:ctx.url,referrer:ctx.referrer},props||{}))};

function identify(id,traits){userId=id,track("$identify",traits||{});try{fetch(ENDPOINT+"/identify",{method:"POST",headers:{"Content-Type":"application/json","X-Reactor-Project-Key":KEY},body:JSON.stringify({anonymous_id:anonId,user_id:id,traits:traits||{}})})}catch(e){}};

function alias(prevId,newId){track("$alias",{previousId:prevId,userId:newId})};

function reset(){userId=void 0,anonId=generateId(),sessionId=generateId(),"undefined"!=typeof localStorage&&localStorage.setItem(STORAGE_KEY,anonId)};

function flush(){if(queue.length>0){var events=queue.splice(0,queue.length),body={events:events.map(function(e){return{event:e.event,anonymous_id:e.anonymousId,user_id:e.userId,session_id:e.sessionId,timestamp:e.timestamp,properties:e.properties,context:e.context}})};"undefined"!=typeof navigator&&navigator.sendBeacon&&"hidden"===document.visibilityState?navigator.sendBeacon(ENDPOINT+"/batch",new Blob([JSON.stringify(body)],{type:"application/json"})):fetch(ENDPOINT+"/batch",{method:"POST",headers:{"Content-Type":"application/json","X-Reactor-Project-Key":KEY},body:JSON.stringify(body),keepalive:!0}).catch(function(){})}};

function optOut(){optedOut=!0,queue=[],fetch(ENDPOINT+"/consent/opt-out",{method:"POST",headers:{"Content-Type":"application/json","X-Reactor-Project-Key":KEY},body:JSON.stringify({anonymous_id:anonId})}).catch(function(){})};

function optIn(){optedOut=!1,fetch(ENDPOINT+"/consent/opt-in",{method:"POST",headers:{"Content-Type":"application/json","X-Reactor-Project-Key":KEY},body:JSON.stringify({anonymous_id:anonId})}).catch(function(){})};

function hashString(s){for(var h=0,i=0;i<s.length;i++)h=((h<<5)-h)+s.charCodeAt(i),h|=0;return h.toString(36)};

function shouldTrackError(fp){var now=Date.now(),last=errorFingerprints[fp];return!(last&&now-last<ERROR_DEDUPE_MS)&&(errorFingerprints[fp]=now,!0)};

{{PAGEVIEW_SETUP}}
{{ERROR_SETUP}}
{{CAPTURE_SETUP}}

flushTimer=setInterval(flush,FLUSH_INTERVAL),"undefined"!=typeof window&&window.addEventListener("beforeunload",flush);

window.ReactorAnalytics={track:track,page:page,identify:identify,alias:alias,reset:reset,flush:flush,optOut:optOut,optIn:optIn,getAnonymousId:function(){return anonId},getUserId:function(){return userId},getSessionId:function(){return sessionId}};
}();
"#;

const PAGEVIEW_SETUP: &str = r#"page();if("undefined"!=typeof window){var pushState=history.pushState,replaceState=history.replaceState;history.pushState=function(){pushState.apply(history,arguments),page()},history.replaceState=function(){replaceState.apply(history,arguments),page()},window.addEventListener("popstate",function(){page()})}"#;

const ERROR_SETUP: &str = r#"if("undefined"!=typeof window){window.addEventListener("error",function(e){var fp=hashString(e.message+"|"+(e.filename||"")+"|"+(e.lineno||0));shouldTrackError(fp)&&track("$error",{message:e.message,filename:e.filename,lineno:e.lineno,colno:e.colno,fingerprint:fp})}),window.addEventListener("unhandledrejection",function(e){var fp=hashString("Unhandled Promise Rejection|"+String(e.reason)+"|0");shouldTrackError(fp)&&track("$error",{message:"Unhandled Promise Rejection",reason:String(e.reason),fingerprint:fp})})}"#;

const CAPTURE_SETUP: &str = r##"if("undefined"!=typeof document){document.addEventListener("click",function(e){var t=e.target;if(t){var el=t.closest("a,button,input[type='submit'],[data-reactor-capture]");if(el){var tag=el.tagName.toLowerCase(),id=el.id?"#"+el.id:"",cls=el.className&&"string"==typeof el.className?"."+el.className.trim().split(/\s+/).slice(0,2).join("."):"",text=(el.textContent||"").trim().slice(0,100)||el.getAttribute("aria-label")||"";track("$autocapture",{event_type:"click",tag_name:tag,selector:tag+id+cls,text:text,href:el.href||void 0,id:el.id||void 0})}}},!0)}"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_snippet() {
        let script = generate_snippet("pk_test_123", "/analytics/v1", true, true, false);
        assert!(script.contains("pk_test_123"));
        assert!(script.contains("/analytics/v1"));
        assert!(script.contains("ReactorAnalytics"));
        assert!(script.contains("track"));
        assert!(script.contains("identify"));
    }

    #[test]
    fn test_snippet_without_pageview() {
        let script = generate_snippet("pk_test", "/api", false, false, false);
        assert!(!script.contains("page();if("));
    }

    #[test]
    fn test_snippet_with_autocapture() {
        let script = generate_snippet("pk_test", "/api", false, false, true);
        assert!(script.contains("$autocapture"));
    }
}
