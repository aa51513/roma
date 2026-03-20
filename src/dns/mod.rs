use std::io::{Result, Error, ErrorKind};
use std::net::IpAddr;
use std::sync::{Mutex, OnceLock};
use futures::executor;
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts, NameServerConfig, NameServerConfigGroup};

use crate::config::DnsServerConfig;

// 使用 OnceLock 替代 static mut，确保线程安全
static NAME_SERVER_CONFIGS: OnceLock<Mutex<Vec<NameServerConfig>>> = OnceLock::new();
static STD_ONCE_COUNTER: OnceLock<Mutex<ResolverOpts>> = OnceLock::new();

/// 获取全局 ResolverOpts
fn global_resolver_opts() -> &'static Mutex<ResolverOpts> {
    STD_ONCE_COUNTER.get_or_init(|| Mutex::new(ResolverOpts::default()))
}

/// 获取全局 NameServerConfig
fn global_name_server_configs() -> &'static Mutex<Vec<NameServerConfig>> {
    NAME_SERVER_CONFIGS.get_or_init(|| Mutex::new(vec![]))
}

// 由于 OnceLock 不能用于异步初始化，因此我们依然用 lazy_static! 进行 TokioAsyncResolver 的初始化
use lazy_static::lazy_static;

lazy_static! {
    static ref DNS: TokioAsyncResolver = {
        let config: ResolverConfig = ResolverConfig::from_parts(
            None,
            vec![],
            NameServerConfigGroup::from(global_name_server_configs().lock().unwrap().clone()),
        );
        let resolver_opts = *global_resolver_opts().lock().unwrap();
        TokioAsyncResolver::tokio(config, resolver_opts).unwrap()
    };
}

/// 初始化 DNS 解析器
pub fn init_resolver(resolver_opts: ResolverOpts, dns_servers: Vec<DnsServerConfig>) {
    *global_resolver_opts().lock().unwrap() = resolver_opts;

    let mut name_server_configs = global_name_server_configs().lock().unwrap();
    name_server_configs.extend(dns_servers.into_iter().map(NameServerConfig::from));

    lazy_static::initialize(&DNS);
}

/// 同步解析域名
pub fn resolve_sync(addr: &str) -> Result<IpAddr> {
    executor::block_on(resolve_async(addr))
}

/// 异步解析域名
pub async fn resolve_async(addr: &str) -> Result<IpAddr> {
    let res = DNS
        .lookup_ip(addr)
        .await
        .map_err(|e| Error::new(ErrorKind::Other, e))?
        .into_iter()
        .next()
        .unwrap();
    Ok(res)
}
