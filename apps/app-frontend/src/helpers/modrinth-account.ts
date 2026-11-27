import { ref } from 'vue'

const STORAGE_KEY = 'crakrinth.modrinth_sign_in'

function load() {
	try {
		return localStorage.getItem(STORAGE_KEY) === '1'
	} catch {
		return false
	}
}

export const modrinthSignInEnabled = ref(load())

export function setModrinthSignInEnabled(enabled: boolean) {
	modrinthSignInEnabled.value = enabled
	try {
		localStorage.setItem(STORAGE_KEY, enabled ? '1' : '0')
	} catch {
	}
}
