
## [0.2.0](https://github.com/pkgforge/aeris/compare/v0.1.0...v0.2.0) - 2026-08-15

### ⛰️  Features

- *(adapters)* Run, ask on a terminal, and fill in a thin listing ([#36](https://github.com/pkgforge/aeris/pull/36)) - ([958055e](https://github.com/pkgforge/aeris/commit/958055e42659abe7f7a5cbe6ea158ca84ac66e4b))
- *(adapters)* Read a block of named values as one package ([#34](https://github.com/pkgforge/aeris/pull/34)) - ([f97604f](https://github.com/pkgforge/aeris/commit/f97604f9dd3639900688914cc630f8a39d981935))
- *(adapters)* Act system wide the way each manager expects ([#31](https://github.com/pkgforge/aeris/pull/31)) - ([69e9988](https://github.com/pkgforge/aeris/commit/69e9988dc4b614d92f31129bb96b7ca5d5c1cd5d))
- *(adapters)* Add manifest-driven command adapter, drop WASM and linked soar ([#30](https://github.com/pkgforge/aeris/pull/30)) - ([74ffa63](https://github.com/pkgforge/aeris/commit/74ffa63f7176411227f42fefd1ebdad43c64e8e1))
- *(browse)* Search only the managers you pick - ([5c82eb3](https://github.com/pkgforge/aeris/commit/5c82eb3f16761621b396e9dbd17ae6f8ac648813))
- *(desktop)* Give aeris an icon and a launcher entry - ([bd5a159](https://github.com/pkgforge/aeris/commit/bd5a159b9a6a40fe31777007a948faadeaf2987c))
- *(progress)* Keep what a manager wrote, not just its last line - ([4b4666a](https://github.com/pkgforge/aeris/commit/4b4666a375a8b97d1278c42845a5debe7bd297e9))
- *(registry)* Read adapters from more than one registry - ([beef04f](https://github.com/pkgforge/aeris/commit/beef04f45c896ed724e92657da32dec64936dbf5))
- *(registry)* Keep a copy, refresh on an interval, offer updates ([#32](https://github.com/pkgforge/aeris/pull/32)) - ([17901cd](https://github.com/pkgforge/aeris/commit/17901cdf831bc949926ffcd870b5ff06459c39bd))
- *(search)* Rank results by relevance, not by manager ([#37](https://github.com/pkgforge/aeris/pull/37)) - ([0c8f65c](https://github.com/pkgforge/aeris/commit/0c8f65c6fc70539525e34dec5a17aecea2df42ce))
- *(settings)* Edit the list of registries - ([dbc1d87](https://github.com/pkgforge/aeris/commit/dbc1d87e785dff1c61a4db4af05177af9c7af8f5))
- *(update)* Show what a manager says while it updates everything - ([fff6756](https://github.com/pkgforge/aeris/commit/fff675687c0f46b455036aa01e5447c01f0d9444))
- *(update)* Tell each manager apart by what it can update - ([13434ea](https://github.com/pkgforge/aeris/commit/13434ea8fa2023d2abee9a476081eb525ad104c2))
- *(window)* Draw our own controls where the compositor will not ([#43](https://github.com/pkgforge/aeris/pull/43)) - ([2b3c07f](https://github.com/pkgforge/aeris/commit/2b3c07fbc3de6be2975b4d5f629e53e880168da7))

### 🐛 Bug Fixes

- *(adapters)* Show which manifest version is held - ([136e2c6](https://github.com/pkgforge/aeris/commit/136e2c69a1306b9d431a558347bf175e96eb396d))
- *(adapters)* Tell a manager version from its manifest version - ([0ccc9fa](https://github.com/pkgforge/aeris/commit/0ccc9fac31dc7cf21cc2f635f8c7b95f6cca23c1))
- *(adapters)* Work in whichever scope a manager is installed for ([#35](https://github.com/pkgforge/aeris/pull/35)) - ([9b80f24](https://github.com/pkgforge/aeris/commit/9b80f244f8d12969f4e1fbb5aa6c319f0bd86a7b))
- *(adapters)* Say what a manager needs and what it could do ([#33](https://github.com/pkgforge/aeris/pull/33)) - ([3bd263e](https://github.com/pkgforge/aeris/commit/3bd263e8718068f893f2aef1a19a8f68670e11a1))
- *(browse)* Say a package is held when the manager will not - ([1416ab3](https://github.com/pkgforge/aeris/commit/1416ab30c09bcb82903acbab8c1a9e456500ac63))
- *(browse)* Keep the manager chips on the Sync row - ([d859c16](https://github.com/pkgforge/aeris/commit/d859c16503238941c906b11d667e9365a0d8dc91))
- *(install)* Treat a package that did not go in as a failure ([#38](https://github.com/pkgforge/aeris/pull/38)) - ([639a3fc](https://github.com/pkgforge/aeris/commit/639a3fcd4c9df9683e873ed2f0190db504d40f81))
- *(progress)* Stop an answer coming back as the manager speaking - ([a1bb0e3](https://github.com/pkgforge/aeris/commit/a1bb0e3b924e37ec9284985fc97ec87c85aa76db))
- *(update)* Only claim an update when the version changed - ([c0796bc](https://github.com/pkgforge/aeris/commit/c0796bca502020714dc872ba1db44dda65be5a9a))
- *(update)* Say what is happening while a package updates - ([973d558](https://github.com/pkgforge/aeris/commit/973d55874e33066328ea5e94cdc83cad7bc39246))
- *(window)* Let the window say what it is called - ([993b3fc](https://github.com/pkgforge/aeris/commit/993b3fcaf2b967484327a50d3748f732a6cabe72))
- *(window)* Draw our own controls, since none may be offered - ([10b47de](https://github.com/pkgforge/aeris/commit/10b47de80c3d9be9ce317ecde3d8e2dfdcdfbab2))
- *(xdg)* Search every shared data directory for adapters - ([c543865](https://github.com/pkgforge/aeris/commit/c5438654b11faf1603465047580a9d28816d5f99))

### 📚 Documentation

- Describe registries and the config file - ([8332021](https://github.com/pkgforge/aeris/commit/83320218aec6ecde95a772922bc8a9e5d1ee341b))
- Say what aeris is for, not what it is - ([7554369](https://github.com/pkgforge/aeris/commit/75543695f23be05cb33bc954ad83d74265b6bb70))
- Describe the adapters aeris actually drives ([#40](https://github.com/pkgforge/aeris/pull/40)) - ([d6c124d](https://github.com/pkgforge/aeris/commit/d6c124d3d059945f2d57ddd640ca64ae0765a8ac))

### ⚙️ Miscellaneous Tasks

- *(nightly)* Build only when there is something new to build - ([18c3d3c](https://github.com/pkgforge/aeris/commit/18c3d3c756c2f6e2bfae873654297d1a66ea8f54))
- Slim the nightly build and unbreak its publish ([#44](https://github.com/pkgforge/aeris/pull/44)) - ([22eae0b](https://github.com/pkgforge/aeris/commit/22eae0ba384b3f30a0e9b1492404d0e62ed2b3a8))
- Slim the graphics stack in the release bundle ([#41](https://github.com/pkgforge/aeris/pull/41)) - ([fa41fe3](https://github.com/pkgforge/aeris/commit/fa41fe3afb2321f487d509dd62b1b88914720cff))
- Publish both tar.xz and onelf on nightly and release - ([011fd01](https://github.com/pkgforge/aeris/commit/011fd015bfc0e86fa989c4efb1b5bd8c245e233b))

## [0.1.0](https://github.com/pkgforge/aeris/compare/v0.0.0...v0.1.0) - 2026-07-01

### ⛰️  Features

- *(browse)* Polish view, wire single package Install click - ([cece84e](https://github.com/pkgforge/aeris/commit/cece84e9b7eb214776da0b54f8ea026489039a17))
- *(dashboard)* Polish view with scroll, primary CTA, semibold headings - ([23890cd](https://github.com/pkgforge/aeris/commit/23890cd07c078e4b1edf6f060339f4876a957d57))
- *(installed)* Polish view, wire single package Update click - ([ac95b31](https://github.com/pkgforge/aeris/commit/ac95b3193cdb18ad3cfa007b52ebe64dfcc351a7))
- *(manifest)* Rename diff buckets and sort entries by name - ([59cb368](https://github.com/pkgforge/aeris/commit/59cb36823254253f153e51a72ec221b24bbe24bb))
- *(manifest)* Reload diff when packages.toml changes on disk - ([3bb5198](https://github.com/pkgforge/aeris/commit/3bb5198aaaff0bfec499ad3dd844e7c4d77d2a06))
- *(manifest)* Per package detail panel on the right - ([ab9668e](https://github.com/pkgforge/aeris/commit/ab9668ed24d39a9afd2b43825ba0d362c6c0cb3c))
- *(manifest)* Warn when an entry references a missing profile - ([84a26ef](https://github.com/pkgforge/aeris/commit/84a26efbe8f9c7dd14f33567fe15cb3b2d5c324e))
- *(manifest)* Full package editor with source, build, options - ([c26372b](https://github.com/pkgforge/aeris/commit/c26372ba04ab176523e60135d9ba0d63696b686f))
- *(manifest)* Edit, add, remove, and import installed packages - ([e8ad104](https://github.com/pkgforge/aeris/commit/e8ad104fb6583456e88558d46e26725e99dab6b1))
- *(manifest)* Wire apply with prune toggle and confirm flow - ([4c481f0](https://github.com/pkgforge/aeris/commit/4c481f04d4b558f309250ecc1832ba3a49145d9b))
- *(manifest)* Add read-only Manifest view for soar declarative state - ([cbfc73f](https://github.com/pkgforge/aeris/commit/cbfc73fe0cb3fca18bd551b04e41d361ddcfc907))
- *(settings)* Redesign view with section cards, real switches, hierarchy - ([f2269fc](https://github.com/pkgforge/aeris/commit/f2269fc61a6d66f5cae7fc87bceb46b6107d3616))
- *(text-input)* Multiline mode + use it for build commands - ([02cc2e2](https://github.com/pkgforge/aeris/commit/02cc2e273ac989fce5366cbd25a5b2bc61be10cd))
- *(updates)* Polish view, wire card selection and update click - ([a79d73f](https://github.com/pkgforge/aeris/commit/a79d73f096094e5606049e343db5369baf44586a))
- Focus root for keyboard actions and add native file picker - ([7561148](https://github.com/pkgforge/aeris/commit/7561148a54c4ccdefc82ab9d38150399fdb047bb))
- Keyboard shortcuts for Esc and Enter - ([17bd98f](https://github.com/pkgforge/aeris/commit/17bd98f7232cfb3d7ef552916570d1d640cc70c6))
- Editable text/number/select fields in Settings via edit modal - ([e20b2b0](https://github.com/pkgforge/aeris/commit/e20b2b01197795a76a8512abcc50dcd3305c8838))
- Surface profile switching in the Adapters view - ([5bfdf73](https://github.com/pkgforge/aeris/commit/5bfdf7315607a0669da5205db7ed9661bf776394))
- Track launched processes and add a Stop button - ([a42dba4](https://github.com/pkgforge/aeris/commit/a42dba4d8972a28beed125593bbe6580e909c3af))
- Enumerate executables and prompt picker for multi-binary packages - ([195be88](https://github.com/pkgforge/aeris/commit/195be888e34e00ed707da4249cc5de929adc58f5))
- Wire Run capability with a Run button on installed cards - ([3986cc0](https://github.com/pkgforge/aeris/commit/3986cc09c8fcd7e781eab82643ce367561373311))
- Fetch and display package_detail in Browse panel - ([decf5fa](https://github.com/pkgforge/aeris/commit/decf5facc32641dd99ce5f495c8518f14c781d3a))
- Add Sync button to Browse, Installed, and Updates toolbars - ([64ea8cd](https://github.com/pkgforge/aeris/commit/64ea8cd2019b37e0f8fea4de4b18a9ece3ab5395))
- Remove plugin files when uninstalling a plugin adapter - ([d480c07](https://github.com/pkgforge/aeris/commit/d480c074a42cb2e9ea3a398ae7cb86b1c8cbd83e))
- Surface BatchProgress events in the header - ([2976e35](https://github.com/pkgforge/aeris/commit/2976e3523c176716275e229a227beb9c06a80daf))
- Wire Settings dirty-state, Toggle clicks, and Revert button - ([d8f5fe4](https://github.com/pkgforge/aeris/commit/d8f5fe46cb5d60d750f413c4d33ec76689cdc398))
- Refresh installed list and toast batch operation results - ([dbe05b2](https://github.com/pkgforge/aeris/commit/dbe05b2f9d9735ec5cf66c0db3be48ddea9d934a))
- Surface sync errors as toasts and persist sync_error - ([652eed4](https://github.com/pkgforge/aeris/commit/652eed45468609411530d20338179bb4fe714f12))
- Surface update operation errors as toasts - ([a060f47](https://github.com/pkgforge/aeris/commit/a060f474f54b618f71f422b4156ff112d25a0c51))
- Show target mode in confirmation messages - ([4b74bba](https://github.com/pkgforge/aeris/commit/4b74bba466fd76a2fa36b5c60785c8f1ea2a14e8))
- Confirm before removing installed packages - ([da27af0](https://github.com/pkgforge/aeris/commit/da27af0492f1a726f6ac44abc8852bd70d1f61ef))
- Make mode badge clickable to toggle User/System - ([e37f567](https://github.com/pkgforge/aeris/commit/e37f5676e049ad7bba1a4d07af48879283789f7d))
- Migrate UI framework from iced to gpui - ([e52fb41](https://github.com/pkgforge/aeris/commit/e52fb418676a9767e73d96168cb12e409e7c0500))
- Add batch operations, operation queue, toast notifications, and per-package progress - ([77ad959](https://github.com/pkgforge/aeris/commit/77ad95947ad985e221729ad88d01542afadd12fa))
- Add can_list_updates capability, per-adapter update UI, and version badge fixes - ([ab6c09a](https://github.com/pkgforge/aeris/commit/ab6c09a4f3ddf7906ebf85875467b7f64851284e))
- Add runtime plugin management and adapter enable/disable - ([b196123](https://github.com/pkgforge/aeris/commit/b196123086fed0083259f60844aa254ec5c79b7f))
- Add PackageMode support, adapter badge colors, and multi-adapter UI - ([8ac26ef](https://github.com/pkgforge/aeris/commit/8ac26efce7c84a9c5c5b89cba4e552329086e897))
- Integrate plugin registry, adapter manager, and multi-adapter routing - ([a0950b5](https://github.com/pkgforge/aeris/commit/a0950b569c1e554326da46a154ea1e1771a0b9b1))
- Add WASM adapter system for sandboxed plugins - ([36df1c7](https://github.com/pkgforge/aeris/commit/36df1c7b4facd6e7bdd1b231f5bdc93c16e29131))
- Improve overall user interface - ([35132e9](https://github.com/pkgforge/aeris/commit/35132e9f2a6b12189020939dcf5fd48ed4aa18f7))
- Full system package mode support across all views - ([b615242](https://github.com/pkgforge/aeris/commit/b615242b64da18802fe32596840ef90df62c8eae))
- Add system package support with privilege escalation - ([81ee3fb](https://github.com/pkgforge/aeris/commit/81ee3fbe9471a759cfbd174bf437550a45697d47))
- Add support for config management - ([0be5f03](https://github.com/pkgforge/aeris/commit/0be5f030c5f7d3852fe4124ea9dbb9e41f284f2f))
- Add live progress reporting for package operations - ([ee9dcb6](https://github.com/pkgforge/aeris/commit/ee9dcb637a8570849121129ba5207171bceca5ca))
- Implement install/remove/update operations - ([d0a7fc3](https://github.com/pkgforge/aeris/commit/d0a7fc3660a52e242acd0dba5deef7d92ece4784))
- Implement dashboard, installed, and updates views - ([0c4ad3d](https://github.com/pkgforge/aeris/commit/0c4ad3d15f4d161fc862863ef9384510edb10119))
- Initialize soar adapter with package listing - ([77a9d65](https://github.com/pkgforge/aeris/commit/77a9d6525b32889fafb9a863fa5f9228691b42c4))
- Add core types, adapter system, and GUI shell - ([e890fee](https://github.com/pkgforge/aeris/commit/e890fee87a3d84554178a51dd746fce3b810e7f6))
- Add theme toggle - ([d384567](https://github.com/pkgforge/aeris/commit/d384567e5d9c8bf27106472729c557e03682632e))
- Setup - ([2ec54b7](https://github.com/pkgforge/aeris/commit/2ec54b783073bec93bc8893dd42418773e4fe86b))

### 🐛 Bug Fixes

- *(layout)* Allow flex content to shrink so panels and rows do not clip - ([b5d7340](https://github.com/pkgforge/aeris/commit/b5d734030e902f992c876f6bb5b1a051361b94a8))
- *(manifest)* Always show undeclared packages regardless of prune toggle - ([4184934](https://github.com/pkgforge/aeris/commit/4184934c6fc46e16d9152e3c26c2925cc80193eb))
- *(manifest)* Strip soar's status suffix before edit/remove - ([1a6c81a](https://github.com/pkgforge/aeris/commit/1a6c81a0c0d73151f92685aea1308e6c222c405d))
- *(manifest)* Read via PackagesConfig and write without losing unknown fields - ([2aabfd4](https://github.com/pkgforge/aeris/commit/2aabfd4aac144f1181fbbdeb4410893abd6bb3f3))
- *(progress)* Route progress events to the Updates view too - ([d134515](https://github.com/pkgforge/aeris/commit/d134515240928b9d4f134a49c7df3811988cd336))
- *(settings)* Unbreak scroll by clearing flex min-height up the chain - ([74abf8b](https://github.com/pkgforge/aeris/commit/74abf8bb42358fb6f530af34da6df84a077f4666))
- *(settings)* Add cancel for select edit modal - ([694f196](https://github.com/pkgforge/aeris/commit/694f196a3c7548ca06d9d0baacae0facdfe32500))
- *(settings)* Make the page scrollable - ([a59abdc](https://github.com/pkgforge/aeris/commit/a59abdc3fba656a6835cd0e4c1a286caa0824321))
- *(text-input)* Scroll horizontally so the caret stays in view - ([23019ff](https://github.com/pkgforge/aeris/commit/23019ff88d8b0d2074ffacb3128cf40948ffa6c1))
- Drop unused bindings flagged by the compiler - ([a74906e](https://github.com/pkgforge/aeris/commit/a74906e0080effa3013d24597aae28dcf19f6b66))
- Widen settings edit modal and clip TextInput overflow - ([0b94dd7](https://github.com/pkgforge/aeris/commit/0b94dd746365656ebf16f0ced2d4dd40557ac8c1))
- Enumerate package binaries via profile bin symlinks and occlude overlays - ([ff9d9c0](https://github.com/pkgforge/aeris/commit/ff9d9c06dca6c7516b25f006dc85b0b5997e482b))
- Run installed binary directly and stop card-level click propagation - ([4f70b80](https://github.com/pkgforge/aeris/commit/4f70b80b20749c165d701ecc942a86ed3d94e562))
- Disambiguate installed packages with same name - ([cb0ed3c](https://github.com/pkgforge/aeris/commit/cb0ed3c6408c863f0b59fa06f6d1b7a9f86359fa))
- Recreate SoarContext after repo enable/disable toggle - ([993525f](https://github.com/pkgforge/aeris/commit/993525f17741b5b6a815426f929d0df5232c53ec))
- Handle install error properly - ([3abfdfa](https://github.com/pkgforge/aeris/commit/3abfdfa0f16ad8a2436d1828a5898c4bc4e82f33))

### 🚜 Refactor

- Move repositories view into per-adapter section - ([d89f9d7](https://github.com/pkgforge/aeris/commit/d89f9d789549d48765d155aceb32e1e26b6c74f4))

### 📚 Documentation

- Rewrite README for scope, install, and features - ([ae83186](https://github.com/pkgforge/aeris/commit/ae831869b3d64161cbcfbfdccb6f57ec16d40ad4))
- Add README - ([f2f9098](https://github.com/pkgforge/aeris/commit/f2f909895b083095bac992449d6c2e4aa932e476))

### ⚙️ Miscellaneous Tasks

- Add release-plz and onelf portable release - ([b1b0fe4](https://github.com/pkgforge/aeris/commit/b1b0fe4c051f6c6abd44833ad20e7d28360321bb))
- Use published soar crates for crates.io publishing - ([4f1ec79](https://github.com/pkgforge/aeris/commit/4f1ec79872397cf657cea6ba34056cd99313ea26))
- Prune unused imports, locals, and design tokens - ([d082e5f](https://github.com/pkgforge/aeris/commit/d082e5f3dcf454f22a33fa10bc63f6e8b251e681))
- Drop musl targets from nightly matrix - ([60f3bee](https://github.com/pkgforge/aeris/commit/60f3bee27b326d1314001fe635471a2f8b4fc4e9))
- Build with cargo-zigbuild to provide a musl C++ toolchain - ([29af431](https://github.com/pkgforge/aeris/commit/29af4316f436eafc88015477aa9fdf00b54d3537))
- Remove dead state fields and types - ([25eae5a](https://github.com/pkgforge/aeris/commit/25eae5ad083a021db1f0300e984a5f03681aa299))
- Fix workflow - ([d48f34a](https://github.com/pkgforge/aeris/commit/d48f34a6a3f5473c9a78f88f3f232677dbdf5da5))
- Fix musl build - ([ee1e344](https://github.com/pkgforge/aeris/commit/ee1e34408b9d96e9cd2ebba69709d7602d9c89d5))
- Release for both gnu and musl - ([dc358ec](https://github.com/pkgforge/aeris/commit/dc358ecc736846ff7e6f36394ba3a44c18cfe25f))
- Add nightly workflow - ([1a138f7](https://github.com/pkgforge/aeris/commit/1a138f7a9b4425b75e1be2cdf0dd550ffb679800))
- Use soar packages from git - ([b0a5edf](https://github.com/pkgforge/aeris/commit/b0a5edff8d4f2f2194441d4a08088b1aa39a1eb8))
# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com),
and this project adheres to [Semantic Versioning](https://semver.org).
