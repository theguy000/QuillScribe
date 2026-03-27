import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'

// Disable right-click context menu
document.addEventListener('contextmenu', (e) => e.preventDefault())

// Remove the inline loading indicator before mounting the app
const loader = document.getElementById('startup-loader')
if (loader) loader.remove()

const app = mount(App, {
  target: document.getElementById('app'),
})

export default app
