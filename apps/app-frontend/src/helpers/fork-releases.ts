import { getCurrentWindow } from '@tauri-apps/api/window'
import { ref } from 'vue'

import {
	checkAppVersion,
	enqueueUpdateForInstallation,
	setRestartAfterPendingUpdate,
} from '@/helpers/utils.js'

const RELEASES_URL = 'https://api.github.com/repos/mocny-gaz/crakrinth/releases?per_page=100'
const TAG_PREFIX = 'crak-v'
const STORAGE_KEY = 'fork_releases'

export type ForkRelease = {
	version: string
	url: string
	notes: string
}

type GithubRelease = {
	tag_name: string
	body: string | null
	draft: boolean
	assets: { name: string; browser_download_url: string }[]
}

/** "0.17.10+2" is build 2 of base version 0.17.10. A plain "0.17.10" is build 0. */
export function parseAppVersion(version: string) {
	const [base, build = ''] = version.split('+')
	return { base, build: /^\d+$/.test(build) ? Number(build) : 0 }
}

/** Release tags write build 2 of 0.17.10 as crak-v0.17.10.2, because a plus sign is awkward in URLs and file names. */
function tagToAppVersion(tagVersion: string) {
	const match = tagVersion.match(/^(\d+\.\d+\.\d+)\.(\d+)$/)
	return match ? `${match[1]}+${match[2]}` : tagVersion
}

export function compareAppVersions(a: string, b: string) {
	const left = parseAppVersion(a)
	const right = parseAppVersion(b)
	return (
		left.base.localeCompare(right.base, undefined, { numeric: true }) || left.build - right.build
	)
}

function loadCached(): ForkRelease[] {
	try {
		const cached = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]')
		return Array.isArray(cached) ? cached : []
	} catch {
		return []
	}
}

/** Shared by the Updates tab so the check button and the version picker show the same list. */
export const forkReleases = ref<ForkRelease[]>(loadCached())

export async function refreshForkReleases() {
	const response = await fetch(RELEASES_URL)
	if (!response.ok) throw new Error(`GitHub returned ${response.status}`)
	const releases: GithubRelease[] = await response.json()
	const found: ForkRelease[] = []
	for (const release of releases) {
		if (release.draft || !release.tag_name.startsWith(TAG_PREFIX)) continue
		const asset = release.assets.find((item) => item.name === 'latest.json')
		if (!asset) continue
		found.push({
			version: tagToAppVersion(release.tag_name.slice(TAG_PREFIX.length)),
			url: asset.browser_download_url,
			notes: (release.body ?? '').trim(),
		})
	}
	found.sort((a, b) => compareAppVersions(b.version, a.version))
	forkReleases.value = found
	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(found))
	} catch {
		// the list still works without the cache
	}
	return found
}

/** Newer builds of the same base version, newest first. These carry new fork features. */
export function newerBuildsOfVersion(releases: ForkRelease[], current: string) {
	const now = parseAppVersion(current)
	return releases
		.filter((release) => {
			const other = parseAppVersion(release.version)
			return other.base === now.base && other.build > now.build
		})
		.sort((a, b) => compareAppVersions(b.version, a.version))
}

export async function installForkRelease(release: ForkRelease) {
	const update = await checkAppVersion(release.url)
	await enqueueUpdateForInstallation(update.rid)
	await setRestartAfterPendingUpdate(true)
	await getCurrentWindow().close()
}
