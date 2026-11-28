// The Home window (home.rs): the sidebar, Home, History and Dictionary, and Settings as a sheet.
import { mount } from 'svelte';
import '../lib/ui/tokens.css';
import '../lib/ui/components.css';
import './home.css';
import Home from './Home.svelte';

mount(Home, { target: document.body });
