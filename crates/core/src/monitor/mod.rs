//! 监控子系统：网络流量采集、分钟级历史存储。
//! 独立于使用统计引擎，由桥接层启动；采集线程 1s 常开，失败静默降级。

pub mod core;
pub mod net;
pub mod store;

pub use core::MonitorCore;
pub use net::NetworkSnapshot;
