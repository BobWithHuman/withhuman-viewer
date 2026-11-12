# WithHuman Viewer

[WithHuman](https://github.com/BobWithHuman/withhuman-core) 엔진이 기록한 실행 그래프를 보는 데스크톱 앱입니다.
에이전트의 요청, 의도, 도구 호출(Call)과 그 결과를 SubGraph로 묶어 보여 주고, 각 Call이 어디서 비롯됐는지
(원인선)와 엔진의 판정(허용·확인·차단)을 따라갈 수 있습니다.

- macOS 11 이상, Apple Silicon과 Intel 모두 지원(유니버설)
- Neo4j에 기록된 실제 세션을 읽거나, Neo4j 없이 **데모 데이터**로 둘러볼 수 있습니다.

## 설치

[Releases](https://github.com/BobWithHuman/withhuman-viewer/releases)에서 `.dmg`를 받아 앱을 응용 프로그램 폴더로 옮깁니다.

## 연결

처음 실행하면 연결 설정(⌘,)에서 Neo4j 주소·데이터베이스·사용자·비밀번호를 넣거나 "데모로 둘러보기"를 고릅니다.
비밀번호는 macOS 키체인에 저장되고, 설정 파일에는 쓰지 않습니다.

## 개발

```sh
cargo run                      # 실행
VIEWER_MOCK=1 cargo run        # 데모 데이터로 실행
cargo test                     # 테스트
cargo packager --release --target universal-apple-darwin   # 유니버설 .app/.dmg (dist/)
```

- 데모 데이터 `mock/fixture.json`은 로컬에 띄운 엔진이 기록한 세션을 앱의 쿼리로 내보낸 것입니다. 다시 만드는 방법은 [mock/README.md](mock/README.md)에 있습니다. `VIEWER_FIXTURE=<파일>`로 다른 파일을 쓸 수 있습니다.
- macOS에서 `cargo run`은 앱을 띄우기 전에 키체인의 Developer ID 인증서(또는 `VIEWER_SIGN_IDENTITY`)로 서명합니다(`scripts/dev-run.sh`). 서명이 빌드마다 같아서, 저장한 Neo4j 비밀번호의 키체인 확인 창이 처음 한 번만 뜹니다. 인증서가 없으면 서명 없이 실행합니다.
- 화면 구성과 진행 계획은 [DESIGN.md](DESIGN.md)에 있습니다.
- 엔진 쪽 브라우저용 뷰어(`withhuman-core`의 `tools/graph-viewer`)는 개발용으로 남아 있고, 같은 쿼리로 같은 형식을 읽습니다.

## 보안

취약점은 공개 이슈 대신 [비공개 제보](https://github.com/BobWithHuman/withhuman-viewer/security/advisories/new)로 알려 주세요.
