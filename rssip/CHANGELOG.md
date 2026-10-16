# 0.1.1 (October 16, 2026)

### Added

* session: added new trait `SessionEventHandler`
* rssip: added changelog files

### Removed

* rssip: removed unused dependency `hickory-resolver`

### Changed

* session: restructured session module
* session: renamed function `receive_dialog_message` to `handle_in_dialog_request` for clarity
* rssip: disabled default features of `futures-util`

### Fixed

* session: added missing content type on accept response

# 0.1.0 (September 15, 2026)

* Initial release.