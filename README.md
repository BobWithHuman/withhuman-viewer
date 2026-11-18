# WithHuman Viewer

[WithHuman](https://github.com/BobWithHuman/withhuman-core) 엔진이 기록한 에이전트 세션을 보여주는 데스크톱 앱입니다.
도구 호출(Call) 하나하나가 사람의 요청을 따른 것인지, 도구 결과 속 글을 따른 것인지 그래프로 보여주며, 엔진의 판정
(허용·확인·차단)과 차단된 호출의 원인 경로를 따라갈 수 있습니다.

## 설치

[Releases](https://github.com/BobWithHuman/withhuman-viewer/releases)에서 `.dmg`를 받아 앱을 응용 프로그램 폴더로 옮깁니다.

### 실행

```sh
git clone https://github.com/BobWithHuman/withhuman-viewer.git
cd withhuman-viewer
cargo admin #보안 담당자 단말
cargo run   #개발자 단말
```

## 보안

취약점은 공개 이슈 대신 [비공개 제보](https://github.com/BobWithHuman/withhuman-viewer/security/advisories/new)로 알려 주세요.
