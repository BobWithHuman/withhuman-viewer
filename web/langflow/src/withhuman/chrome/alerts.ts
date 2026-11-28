// Langflow's toasts, cut to what a read-only record viewer should say. Sessions are never saved,
// built, uploaded or updated, so Langflow's notices about that ("Flow saved successfully!" when a
// canvas is left, auto-save, build errors from the absent backend, store and version checks) are
// dropped at the store: the setters every component reads from `useAlertStore` are wrapped once.
//   success / notice -> only a copy ("복사됨", Langflow's own "...copied" keys in Korean);
//   error            -> kept only when it is Korean and not about saving, building, the store,
//                       updates or the backend.

import useAlertStore from "@/stores/alertStore";

const HANGUL = /[가-힣]/;
const COPY = /복사/;
const NOISE = /sav|build|store|updat|upgrad|upload|import|export|download|version|backend|langflow|저장|빌드|스토어|업데이트|업그레이드|업로드|가져오|내보내|다운로드|버전|백엔드/i;

type Msg = { title: string; list?: string[] };
const text = (m: Msg) => [m.title, ...(m.list ?? [])].join(" ");

const quiet = {
  success: (m: Msg) => !COPY.test(m.title),
  notice: (m: Msg) => !COPY.test(m.title),
  error: (m: Msg) => !HANGUL.test(m.title) || NOISE.test(text(m)),
};

const s = useAlertStore.getState();
const { setSuccessData, setNoticeData, setErrorData } = s;
useAlertStore.setState({
  setSuccessData: (m) => (quiet.success(m) ? undefined : setSuccessData(m)),
  setNoticeData: (m) => (quiet.notice(m) ? undefined : setNoticeData(m)),
  setErrorData: (m) => (quiet.error(m) ? undefined : setErrorData(m)),
});
// Anything that reached the lists before the wrap (or through `addNotificationToTempList`
// directly) is filtered the same way.
const pass = (n: { type?: string; title: string; list?: string[] }) =>
  !(n.type === "error" ? quiet.error(n) : quiet.success(n));
useAlertStore.subscribe((st, prev) => {
  if (st.tempNotificationList === prev.tempNotificationList) return;
  const kept = st.tempNotificationList.filter(pass);
  if (kept.length !== st.tempNotificationList.length) useAlertStore.setState({ tempNotificationList: kept });
});
