# Changelog

# v0.1.0

This is the first `vm-superio-ser` release.
The `vm-superio-ser` crate provides support for persisting the states from
`vm-superio`. For now, it provides this support only for the `Rtc` device.
`RtcStateSer` can be used by customers who need an `RtcState` that is also
`(De)Serialize` and/or `Versionize`.
This version of `RtcStateSer` is compatible with v0.5.0 version of `RtcState`.
