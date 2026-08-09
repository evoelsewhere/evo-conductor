const TOKEN_KEY = "conductor.token"
const USER_KEY = "conductor.user"

function migrateLegacyStorage() {
  const legacyToken = localStorage.getItem(TOKEN_KEY)
  const legacyUser = localStorage.getItem(USER_KEY)
  if (legacyToken && !sessionStorage.getItem(TOKEN_KEY)) {
    sessionStorage.setItem(TOKEN_KEY, legacyToken)
  }
  if (legacyUser && !sessionStorage.getItem(USER_KEY)) {
    sessionStorage.setItem(USER_KEY, legacyUser)
  }
  localStorage.removeItem(TOKEN_KEY)
  localStorage.removeItem(USER_KEY)
}

export const authSession = {
  getToken() {
    migrateLegacyStorage()
    return sessionStorage.getItem(TOKEN_KEY)
  },
  getUser() {
    migrateLegacyStorage()
    return sessionStorage.getItem(USER_KEY)
  },
  setToken(token: string) {
    sessionStorage.setItem(TOKEN_KEY, token)
  },
  set(token: string, user: unknown) {
    sessionStorage.setItem(TOKEN_KEY, token)
    sessionStorage.setItem(USER_KEY, JSON.stringify(user))
  },
  clear() {
    sessionStorage.removeItem(TOKEN_KEY)
    sessionStorage.removeItem(USER_KEY)
    localStorage.removeItem(TOKEN_KEY)
    localStorage.removeItem(USER_KEY)
  },
}
