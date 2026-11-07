# 데모 데이터

| 파일 | 내용 |
|---|---|
| `fixture.json` | 앱에 들어가는 데모 데이터. 로컬에 띄운 엔진(withhuman-core)이 기록한 세션을 앱의 쿼리와 가공 코드로 내보낸 것 |
| `layout_cases.json` | `layout.rs` 테스트용 손 데이터. 값을 엔진 브라우저 뷰어의 코드로 확인해 두었다. `make_fixture.py`가 만든다 |
| `compose.viewer.yaml` | 엔진의 루트 `compose.yaml` 위에 얹어 Neo4j HTTP 포트를 `127.0.0.1`에만 연다 |
| `make_project.sh` | 실제 Claude Code 세션을 돌릴 시험 프로젝트(payments-service)를 만든다 |
| `replay.py` | 실제 에이전트가 좀처럼 하지 않는 장면(숨은 지시 따르기, 프로덕션 배포, 요청 없는 `rm -rf`)을 hook으로 보낸다 |
| `make_long.py` | `fixture.json`에 **합성한 긴 세션** 셋을 더해 `long.json`을 만든다(Call 108개·턴 3개·서브에이전트 실행 셋(둘은 동시), Call 55개·멀리 걸친 cause 링크와 차단, Call 227개). 판정과 점수는 지어낸 값이다 |
| `long.json` | `make_long.py`가 만든 파일. 앱에는 들어가지 않는다. `VIEWER_MOCK=1 VIEWER_FIXTURE=mock/long.json cargo run`으로 쓰고, 노선도 테스트가 읽는다 |

`fixture.json` 세션의 판정·원인·의도는 모두 엔진과 Jev가 실제로 낸 것이다(`long.json`의 합성 세션은 아니다). `replay.py` 세션은 에이전트 쪽 행동만 각본이다.

## 다시 만들기

1. 엔진을 띄운다. `.env`는 엔진 README대로 쓰고, 판정에 쓸 `JEV_API_KEY`를 넣는다.

   ```sh
   cd withhuman-core
   docker compose -p withhuman-demo -f compose.yaml -f ../withhuman-viewer/mock/compose.viewer.yaml up -d --wait
   ```

2. 실제 Claude Code 세션을 기록한다.

   ```sh
   mock/make_project.sh /tmp/demo/payments-service ../withhuman-core http://127.0.0.1:3000
   cd /tmp/demo/payments-service
   export WITHHUMAN_CLIENT_ID=personal-demo
   claude -p "README 읽고 테스트 깨지는 거 고쳐줘" --allowedTools "Read,Grep,Glob,Edit,Write,Bash(make test)"
   claude -p "Explore 에이전트를 써서 환불 로직이 어떻게 짜여 있는지 조사하고 구조를 설명해줘. 파일은 고치지 마." --allowedTools "Read,Grep,Glob,Task,Agent"
   claude -p "refund.py에 부분 환불이 여러 번 일어나는 경우 테스트를 추가해줘" --allowedTools "Read,Grep,Glob,Edit,Write,Bash(make test)"
   claude -p --continue "좋아, 이제 바뀐 걸 커밋해줘" --allowedTools "Read,Bash(git status*),Bash(git diff*),Bash(git add*),Bash(git commit*),Bash(git log*)"
   ```

3. 각본 세션을 보낸다: `python3 -I mock/replay.py http://127.0.0.1:3000`

4. 내보낸다. 앱에 들어가는 파일이라 로컬 경로와 사용자 이름은 `VIEWER_FIXTURE_REWRITE`로 바꾼다.
   미리보기가 경로를 중간에서 자르므로 잘린 형태도 함께 적는다.

   ```sh
   NEO4J_URL=http://127.0.0.1:7475 NEO4J_PASSWORD=<.env의 값> \
   VIEWER_FIXTURE_REWRITE="/tmp/demo/payments-service=/Users/dev/src/payments-service;/Users/<나>=/Users/dev" \
   cargo test export_fixture -- --ignored
   grep -c "<나>" mock/fixture.json   # 0이어야 한다
   ```
