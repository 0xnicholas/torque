---
layout: page
---

<script setup>
import { ApiReference } from '@scalar/api-reference'
import '@scalar/api-reference/style.css'
import { useData } from 'vitepress'
import { ref, watch } from 'vue'

const { isDark } = useData()
const show = ref(true)

watch(isDark, () => {
  show.value = false
  setTimeout(() => { show.value = true }, 0)
})

const customCss = `
  /* 字体 */
  .scalar-app {
    font-family: -apple-system, BlinkMacSystemFont, 'Inter', sans-serif;
  }

  /* 左侧侧边栏 */
  .sidebar {
    padding: 16px 8px !important;
  }
  .sidebar-heading {
    font-size: 11px !important;
    font-weight: 600 !important;
    letter-spacing: 0.06em !important;
    text-transform: uppercase !important;
    opacity: 0.5 !important;
    margin: 16px 0 6px 8px !important;
  }
  .sidebar-item {
    border-radius: 6px !important;
    font-size: 13px !important;
  }
  .sidebar-item:hover {
    background: var(--scalar-background-2) !important;
  }

  /* 端点 badge */
  .httpMethod {
    font-size: 10px !important;
    font-weight: 700 !important;
    padding: 2px 6px !important;
    border-radius: 4px !important;
    letter-spacing: 0.04em !important;
  }

  /* 主内容区 */
  .section-header h2 {
    font-size: 22px !important;
    font-weight: 600 !important;
    letter-spacing: -0.02em !important;
  }
  .endpoint-path {
    font-size: 13px !important;
    opacity: 0.8 !important;
  }

  /* 代码块 */
  .code-block {
    border-radius: 8px !important;
    font-size: 12.5px !important;
  }

  /* 去掉多余分割线 */
  .section-divider {
    opacity: 0.15 !important;
  }
  
  .scalar-mcp-layer {
    display: none !important;
  }
`
</script>

<ClientOnly>
  <ApiReference
    v-if="show"
    :configuration="{
      spec: { url: '/torque-v1.yaml' },
      theme: 'default',
      darkMode: isDark,
      hideModels: false,
      hiddenClients: ['ruby', 'php'],
      hideDownloadButton: true,
      customCss,
    }"
  />
</ClientOnly>
