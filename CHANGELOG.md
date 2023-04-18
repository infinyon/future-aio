# Release Notes

## 0.5.0
* Move to `memmap2` because `memmap` is unmaintained.
* Enable `rustls` on `windows`

([#206](https://github.com/infinyon/future-aio/pull/206))

## 0.4.5
* Doomsday timer to ensure crash ([#196](https://github.com/infinyon/future-aio/pull/196))

## 0.4.4
* update rustls

## 0.4.3
* expose futures

## 0.4.2
* re-export async_std::sync structs

## 0.4.1
* Make tcp connector `Send` for `wasm32` ([#165](https://github.com/infinyon/future-aio/pull/165))

## 0.4.0
* Upgrade Rustls results in AcceptorBuilder and ConnectorBuilder API changes ([#151](https://github.com/infinyon/future-aio/pull/154))

## 0.3.18
* Add async retries ([#151](https://github.com/infinyon/future-aio/pull/151))

## 0.3.17
* Added wasm32 `Send + Sync` contraints for `Connection` ([#149](https://github.com/infinyon/future-aio/pull/149/))

## 0.3.14 - 2022-03-08
* Add Intermediate CA Trust Anchor ([#135](https://github.com/infinyon/future-aio/issues/135))

## 0.3.13
* Fixed another zero copy bug ([#127](https://github.com/infinyon/future-aio/issues/127))

## 0.3.9
* Fix zero copy bug ([#96](https://github.com/infinyon/future-aio/pull/91))

## 0.3.8
* Fix zero copy bug ([#91](https://github.com/infinyon/future-aio/pull/91))
## 0.3.7
* Add native2_tls support for windows. ([#85](https://github.com/infinyon/future-aio/pull/85))

## 0.3.6 - 2021-07-19
* OpenSSL is now vendored by default ([#80](https://github.com/infinyon/future-aio/pull/80))
* Added AsConnectionFd trait to retrieve windows and unix sockets. ([#80](https://github.com/infinyon/future-aio/pull/80))
* Added windows support. ([#80](https://github.com/infinyon/future-aio/pull/80))

## 0.3.2 - 2020-05-20
* Expose spawn_local as unstable ([#69](https://github.com/infinyon/fluvio/pull/69))
* Use platform independent File Descriptor ([#67](https://github.com/infinyon/fluvio/pull/67))

