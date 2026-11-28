// WithHuman's Korean for Langflow's i18n keys that our screens can show: short nouns (the product
// shows numbers, labels and chips, never sentences), "" where Langflow writes a description line,
// and no "Langflow". Keys not listed keep Langflow's own ko.json.

export const KO: Record<string, string> = {
  // The stepper modal's progress (the first launch): the count alone.
  "stepper.completed": "{{current}}/{{total}}",
  // Canvas controls (zoom menu, note, minimize) on a locked session.
  "canvas.controls": "캔버스",
  "version.readOnly": "읽기 전용",
  "flow.canvasLabel": "캔버스",
  "loading.loading": "불러오는 중",
  // A node's output button: sessions are never built.
  "node.buildComponentFirst": "출력 없음",
  "node.outputCantBeDisplayed": "출력 없음",
  "node.runComponent": "실행",
  // Langflow's own list-page labels, should any still be reached.
  "mainPage.tabFlows": "기록",
  "mainPage.searchPlaceholder": "검색",
  "flows.viewList": "목록",
  "flows.viewGrid": "카드",
  "flows.openFlow": "열기",
  "mainPage.editedAgo": "{{time}}",
  "emptyPage.description": "",
  "emptyPage.startBuilding": "세션 없음",
  "emptyPage.emptyProject": "세션 없음",
  "page.welcomeDescription": "",
  "page.emptyFolder": "",
  // The search fields' empty results.
  "sidebar.noComponentsFound": "결과 없음",
  "sidebar.clearSearch": "지우기",
  "sidebar.tryDifferentQuery": "",
  // Settings frame (title only, no description line).
  "settings.description": "",
  "settings.generalDescription": "",
  "settings.languageDescription": "",
  // Unsaved-changes dialog (sessions are read-only; shown only if auto-save were off).
  "flow.unsavedChangesLost": "",
  "flow.enableAutoSaving": "",
  "flow.toAvoidLosingProgress": "",
  // Things that name Langflow.
  "assistant.title": "어시스턴트",
  "assistant.openAssistant": "어시스턴트",
  "assistant.tryAssistant": "",
  "help.getLangflowDesktop": "",
  "common.langflowLogo": "WithHuman",
  "common.langflowLogoLight": "WithHuman",
  "common.langflowLogoDark": "WithHuman",
  "storageUpgrade.automatic": "",
  // Playground / flow wording in hidden panels and tooltips.
  "misc.addChatInputOutput": "",
  "misc.addChatInputOutputPlayground": "",
  "flow.untitledFlow": "이름 없음",
  "flow.flowSettings": "세션",
  "flow.editDetailsAndSettings": "{{flowName}}",
};
