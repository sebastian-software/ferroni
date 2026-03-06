import { defineConfig } from 'vite'
import { ardo } from 'ardo/vite'

export default defineConfig({
  plugins: [
    ardo({
      title: 'Ferroni',
      description: 'Built with Ardo',

      themeConfig: {
        siteTitle: 'Ferroni',

        nav: [
          { text: 'Guide', link: '/guide/getting-started' }
        ],

        sidebar: [
          {
            text: 'Guide',
            items: [{ text: 'Getting Started', link: '/guide/getting-started' }],
          }
        ],

        footer: {
          message: 'Released under the MIT License.',
        },

        search: {
          enabled: true,
        },
      },
    }),
  ],
})
