<script setup>
import { defineMessages, injectNotificationManager, Toggle, useVIntl } from '@modrinth/ui'
import { ref, watch } from 'vue'

import JavaSelector from '@/components/ui/JavaSelector.vue'
import { get_java_versions, modrinth_java_available, set_java_version } from '@/helpers/jre'
import { get, set } from '@/helpers/settings.ts'
import { useTheming } from '@/store/state'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const themeStore = useTheming()
const settings = ref(await get())
watch(settings, async () => await set(settings.value), { deep: true })
const modrinthJavaAvailable = ref(await modrinth_java_available().catch(() => false))

const messages = defineMessages({
	optimisedTitle: {
		id: 'app.settings.java-installations.optimised.title',
		defaultMessage: 'Optimised Java',
	},
	optimisedDescription: {
		id: 'app.settings.java-installations.optimised.description',
		defaultMessage:
			'Use the Java that Modrinth App already downloaded, so Crakrinth does not take more disk space. When Modrinth App has no Java for a version, Crakrinth downloads its own. Java you pick below is always used first.',
	},
	optimisedUnavailable: {
		id: 'app.settings.java-installations.optimised.unavailable',
		defaultMessage:
			'Not needed, because Modrinth App is not installed here. If it lives in another folder, import from that folder in Settings, Import, and this switch will use its Java.',
	},
	javaLocation: {
		id: 'app.settings.java-installations.location.title',
		defaultMessage: 'Java {version, number} location',
	},
})

const javaVersions = ref(await get_java_versions().catch(handleError))
async function updateJavaVersion(version) {
	if (version?.path === '') {
		version.path = undefined
	}

	if (version?.path) {
		version.path = version.path.replace('java.exe', 'javaw.exe')
	}

	await set_java_version(version).catch(handleError)
}
</script>
<template>
	<div class="flex flex-col gap-6">
		<div
			class="flex items-center justify-between gap-4"
			:class="{ 'opacity-50': !modrinthJavaAvailable }"
		>
			<div>
				<h2 class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.optimisedTitle) }}
				</h2>
				<p class="m-0 mt-1">
					{{
						formatMessage(
							modrinthJavaAvailable ? messages.optimisedDescription : messages.optimisedUnavailable,
						)
					}}
				</p>
			</div>
			<Toggle
				id="optimised-java"
				:disabled="!modrinthJavaAvailable"
				:model-value="modrinthJavaAvailable && themeStore.getFeatureFlag('optimised_java')"
				@update:model-value="
					(value) => {
						themeStore.featureFlags['optimised_java'] = !!value
						settings.feature_flags['optimised_java'] = !!value
					}
				"
			/>
		</div>
		<div
			v-for="(javaVersion, index) in [25, 21, 17, 8]"
			:key="`java-${javaVersion}`"
			class="flex flex-col gap-2.5"
		>
			<h2 class="m-0 text-lg font-semibold text-contrast" :class="{ 'mt-4': index !== 0 }">
				{{ formatMessage(messages.javaLocation, { version: javaVersion }) }}
			</h2>
			<JavaSelector
				:id="'java-selector-' + javaVersion"
				v-model="javaVersions[javaVersion]"
				:version="javaVersion"
				@update:model-value="updateJavaVersion"
			/>
		</div>
	</div>
</template>
