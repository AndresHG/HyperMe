// The Home window (home.rs): the sidebar, Home, History and Dictionary, and Settings as a sheet.
import { mount } from 'svelte';
import '../../../ui/tokens.css';
import '../../../ui/components.css';
import './home.css';
import Home from './Home.svelte';

mount(Home, { target: document.body });
