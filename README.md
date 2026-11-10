# WithHuman Viewer

[WithHuman](https://github.com/BobWithHuman/withhuman-core) 엔진이 기록한 에이전트 세션을 지하철 노선도로 보는 데스크톱 앱입니다.
도구 호출(Call) 하나하나가 사람의 요청을 따른 것인지, 도구 결과 속 글을 따른 것인지 역과 선로로 보여 주고, 엔진의 판정
(허용·확인·차단)과 차단된 호출의 원인 경로를 따라갈 수 있습니다.

- macOS 11 이상, Apple Silicon과 Intel 모두 지원(유니버설)
- Neo4j에 기록된 실제 세션을 읽거나, Neo4j 없이 **데모 데이터**로 둘러볼 수 있습니다.

## 설치

[Releases](https://github.com/BobWithHuman/withhuman-viewer/releases)에서 `.dmg`를 받아 앱을 응용 프로그램 폴더로 옮깁니다.

## 소스에서 실행하기

### 준비물

| 무엇 | 왜 |
|---|---|
| [Rust](https://rustup.rs) 1.88 이상 | 앱을 빌드합니다. |
| Xcode Command Line Tools (`xcode-select --install`) | macOS에서 빌드·서명에 필요합니다. |
| Jev API 키 | 첫 실행 때 등록합니다. 엔진이 호출을 판정할 때 씁니다. |
| [Docker Desktop](https://www.docker.com/products/docker-desktop/)과 [withhuman-core](https://github.com/BobWithHuman/withhuman-core) | 엔진을 이 맥에서 돌릴 때만 필요합니다. 이미 떠 있는 엔진(서버)에 접속하거나 데모로만 볼 거라면 없어도 됩니다. |

### 실행

```sh
git clone https://github.com/BobWithHuman/withhuman-viewer.git
cd withhuman-viewer
cargo run
```

처음 빌드는 몇 분 걸립니다. 그다음부터는 바뀐 부분만 빌드해서 금방 뜹니다.

### 첫 실행

처음 켜면 작은 창에서 두 가지를 정합니다.

1. **Jev API 키**: 넣고 "확인"을 누르면 Jev(`api.typesafe.ai`)에 키가 맞는지 물어봅니다. 질문 없는 요청이라 비용이 들지 않습니다.
   맞는 키만 macOS 키체인에 저장하고, 설정 파일에는 쓰지 않습니다. `~/withhuman-core/.env`에 키가 있으면 "키 넣기"로 바로 채울 수 있습니다.
2. **엔진**: 둘 중 하나를 고릅니다.
   - **이 맥에서 실행**: 엔진 폴더(기본 `~/withhuman-core`)에서 `docker compose up -d`로 엔진을 띄우고, 등록한 키를
     `JEV_API_KEY`로 넘깁니다. `.env`의 `IMAGE_TAG`·`NEO4J_PASSWORD`를 쓰며, 진행 상황이 창에 나옵니다. Neo4j가 응답하면 끝납니다.
   - **서버에 접속**: 이미 떠 있는 엔진의 Neo4j 주소·사용자·비밀번호를 넣습니다. 비밀번호도 키체인에 저장됩니다.

"시작하기"가 성공하면 창이 커지고 메인 페이지(주목할 세션)가 열립니다. 키 없이 보려면 "키 없이 데모로 둘러보기"를 누릅니다.
연결은 나중에 상단바의 "연결 설정…"(⌘,)에서 바꿀 수 있습니다.

### 화면

- **홈**: 차단되거나 확인을 요청한 호출이 있었던 세션을 모아 보여 줍니다. 누르면 그 세션의 노선도가 열립니다.
- **노선도**: 왼쪽 도구 줄(범례, 확대·축소·맞춤, 이전/다음), 가운데 노선도와 그 왼쪽 위의 범례(종류별 개수, 눈 아이콘으로 흐리게, 펼쳐서 하나씩 보기, 강조해서 보기), 오른쪽 자세히 패널로 이루어집니다.
  세션은 상단바의 "바꾸기"로 고르고, ←/→ 키로 프롬프트와 호출을 차례로 따라갑니다. 차단·확인 요청된 호출을 누르면 그 호출을 시킨
  출력까지 거슬러 올라간 길만 색으로 남습니다.
- **기능 A·B**: 곧 추가될 기능의 자리입니다.

### 실행할 때 쓰는 환경 변수

| 변수 | 하는 일 |
|---|---|
| `VIEWER_MOCK=1` | Neo4j 대신 내장 데모 데이터로 엽니다. 첫 실행 설정도 건너뜁니다. |
| `VIEWER_ONBOARD=1` | 이미 설정했어도 첫 실행 화면을 다시 띄웁니다. 메인 페이지 맨 아래 "처음 설정 다시 하기"와 같습니다. |
| `VIEWER_SCREEN=<work\|a\|b\|mcp\|secrets\|inventory>` | 첫 화면을 고릅니다: 노선도(기록), 기능 A(감사), 기능 B(정책), 그리고 보안담당자 앱의 MCP 사용, 민감 정보, 인벤토리. 화면을 눌러 가지 않고 바로 볼 때 씁니다. |
| `VIEWER_TAB=<탭>` | 보안담당자 앱의 첫 탭을 고릅니다. 메인: `detections`, `machines`, `sessions`, `engine`. 감사: `approvals`, `kills`, `integrity`. |
| `VIEWER_NO_UPDATE=1` | 설치된 앱의 자동 업데이트 확인을 끕니다. |
| `WITHHUMAN_CORE=<폴더>` | "이 맥에서 실행"의 엔진 폴더 기본값입니다(없으면 `~/withhuman-core`). |
| `JEV_URL=<주소>` | 키를 확인할 Jev 주소입니다(기본 `https://api.typesafe.ai`). |
| `NEO4J_URL`, `NEO4J_DATABASE`, `NEO4J_USER`, `NEO4J_PASSWORD` | 설정 파일이 없을 때 쓰는 Neo4j 접속 기본값입니다. |
| `VIEWER_FIXTURE=<파일>` | 데모 데이터로 다른 파일을 씁니다. |
| `VIEWER_DB=<주소>` | 이번 실행만 그 데이터베이스를 읽습니다(저장하지 않음). 엔진의 PostgreSQL은 `postgres://사용자:비밀번호@호스트:포트/DB`, 예전 엔진의 Neo4j는 `http://…`. |
| `VIEWER_SIGN_IDENTITY=<인증서 이름>` | `cargo run`이 앱을 서명할 인증서입니다(아래 참고). |

설정은 `~/Library/Application Support/dev.withhuman.viewer/`(`settings.json`, `prefs.json`)에 저장됩니다. 지우면 처음 상태로 돌아갑니다.
비밀번호와 Jev 키는 키체인의 `dev.withhuman.viewer` 항목에 있습니다.

## 개발

```sh
cargo run                      # 실행
VIEWER_MOCK=1 cargo run        # 데모 데이터로 실행
cargo test                     # 테스트
cargo clippy --all-targets     # 린트
cargo packager --release --target universal-apple-darwin   # 유니버설 .app/.dmg (dist/)
```

- macOS에서 `cargo run`은 앱을 띄우기 전에 키체인의 Developer ID 인증서(또는 `VIEWER_SIGN_IDENTITY`)로 서명합니다(`scripts/dev-run.sh`).
  서명이 빌드마다 같아서, 키체인 확인 창이 처음 한 번만 뜹니다. 인증서가 없으면 서명 없이 실행하고, 그때는 빌드할 때마다 키체인이
  비밀번호 사용을 물을 수 있습니다.
- 데모 데이터 `mock/fixture.json`은 로컬에 띄운 엔진이 기록한 세션을 앱의 쿼리로 내보낸 것입니다. 다시 만드는 방법은 [mock/README.md](mock/README.md)에 있습니다.
- 화면 구성과 설계 결정은 [DESIGN.md](DESIGN.md)에 있습니다.
- 엔진 쪽 브라우저용 뷰어(`withhuman-core`의 `tools/graph-viewer`)는 개발용으로 남아 있고, 같은 쿼리로 같은 형식을 읽습니다.

## 보안

취약점은 공개 이슈 대신 [비공개 제보](https://github.com/BobWithHuman/withhuman-viewer/security/advisories/new)로 알려 주세요.
