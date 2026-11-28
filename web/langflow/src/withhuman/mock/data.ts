// Static data for the mock backend: the user, the one project ("기록") and the component
// palette (`/all`). The flows are the app's sessions (../flow/sessions.ts).

import { componentTypes } from "../flow/convert";
import { PROJECT_ID, PROJECT_NAME, USER_ID } from "../flow/sessions";

type Json = any;

export { PROJECT_ID, PROJECT_NAME, USER_ID };

const NOW = new Date().toISOString();

export const USER = {
  id: USER_ID,
  username: "langflow",
  is_active: true,
  is_superuser: true,
  profile_image: null,
  create_at: NOW,
  updated_at: NOW,
  last_login_at: NOW,
  store_api_key: null,
  optins: {
    github_starred: true,
    discord_clicked: true,
    dialog_dismissed: true,
    mcp_dialog_dismissed: true,
  },
};

export function initialProjects(): Json[] {
  return [
    {
      id: PROJECT_ID,
      name: PROJECT_NAME,
      description: "",
      parent_id: null,
      auth_settings: null,
      owner_username: USER.username,
      is_owner: true,
    },
  ];
}

/** `/all`: only the kinds of node a session is drawn with. */
export function buildAllTypes(): Json {
  return componentTypes();
}
