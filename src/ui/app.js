// Global state
let infrastructure = null;
let projects = [];
let allProjects = []; // For search
let currentView = 'dashboard';
let activeWebSocket = null;

// Utility functions
function showLoading() {
    $('#loadingSpinner').removeClass('hidden').addClass('flex');
}

function hideLoading() {
    $('#loadingSpinner').removeClass('flex').addClass('hidden');
}

function showStatus(message, type = 'info') {
    const colors = {
        success: 'bg-green-100 border-green-400 text-green-700',
        error: 'bg-red-100 border-red-400 text-red-700',
        info: 'bg-blue-100 border-blue-400 text-blue-700',
        warning: 'bg-yellow-100 border-yellow-400 text-yellow-700'
    };

    $('#statusMessage')
        .removeClass('hidden')
        .removeClass(Object.values(colors).join(' '))
        .addClass(colors[type])
        .addClass('border px-4 py-3 rounded relative')
        .html(`
            <span class="block sm:inline">${message}</span>
            <span class="absolute top-0 bottom-0 right-0 px-4 py-3 cursor-pointer" onclick="$('#statusMessage').addClass('hidden')">
                <svg class="fill-current h-6 w-6" role="button" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20">
                    <title>Close</title>
                    <path d="M14.348 14.849a1.2 1.2 0 0 1-1.697 0L10 11.819l-2.651 3.029a1.2 1.2 0 1 1-1.697-1.697l2.758-3.15-2.759-3.152a1.2 1.2 0 1 1 1.697-1.697L10 8.183l2.651-3.031a1.2 1.2 0 1 1 1.697 1.697l-2.758 3.152 2.758 3.15a1.2 1.2 0 0 1 0 1.698z"/>
                </svg>
            </span>
        `);

    // Auto-hide after 5 seconds for success messages
    if (type === 'success') {
        setTimeout(() => {
            $('#statusMessage').addClass('hidden');
        }, 5000);
    }
}

function showModal(title, content) {
    $('#modalTitle').text(title);
    $('#modalContent').html(content);
    $('#detailsModal').removeClass('hidden').addClass('flex');
}

function hideModal() {
    $('#detailsModal').removeClass('flex').addClass('hidden');
}

// Navigation
function switchView(view) {
    currentView = view;

    // Update nav buttons
    $('#navDashboard, #navProjects, #navGraph').removeClass('bg-white/20');
    $(`#nav${view.charAt(0).toUpperCase() + view.slice(1)}`).addClass('bg-white/20');

    // Hide all views
    $('#dashboardView, #projectsView, #graphView').addClass('hidden');

    // Show selected view
    $(`#${view}View`).removeClass('hidden');

    // Load view-specific data
    if (view === 'dashboard') {
        loadDashboard();
    } else if (view === 'graph') {
        loadGraph();
    }
}

// Navigation event handlers
$('#navDashboard').on('click', () => switchView('dashboard'));
$('#navProjects').on('click', () => switchView('projects'));
$('#navGraph').on('click', () => switchView('graph'));

// Console Modal with WebSocket
function showConsole(title, initialMessage = '') {
    $('#consoleTitle').text(title);
    $('#consoleOutput').html(initialMessage || '<span class="text-gray-500">Connecting...</span>');
    $('#consoleSpinner').removeClass('hidden');
    $('#consoleStatus').text('Connecting...').removeClass('bg-green-600 bg-red-600').addClass('bg-yellow-600');
    $('#consoleActions').addClass('hidden');
    $('#consoleModal').removeClass('hidden').addClass('flex');
}

function updateConsoleStatus(status, success = null) {
    const $status = $('#consoleStatus');
    $status.text(status);

    if (success === true) {
        $status.removeClass('bg-yellow-600 bg-red-600').addClass('bg-green-600');
    } else if (success === false) {
        $status.removeClass('bg-yellow-600 bg-green-600').addClass('bg-red-600');
    } else {
        $status.removeClass('bg-green-600 bg-red-600').addClass('bg-yellow-600');
    }
}

function appendConsoleOutput(message, type = 'info') {
    const $output = $('#consoleOutput');
    const currentHtml = $output.html();

    // Remove "Connecting..." message if present
    if (currentHtml.includes('Connecting')) {
        $output.html('');
    }

    let colorClass = 'text-green-300';
    if (type === 'error') colorClass = 'text-red-400';
    else if (type === 'warning') colorClass = 'text-yellow-400';
    else if (type === 'success') colorClass = 'text-green-400';
    else if (type === 'section') colorClass = 'text-cyan-400 font-bold';

    $output.append(`<div class="${colorClass}">${escapeHtml(message)}</div>`);

    // Auto-scroll to bottom
    if ($output.length > 0 && $output[0]) {
        $output[0].scrollTop = $output[0].scrollHeight;
    }
}

function finishConsole(success, finalMessage = null) {
    $('#consoleSpinner').addClass('hidden');
    $('#consoleActions').removeClass('hidden');
    updateConsoleStatus(success ? 'Completed' : 'Failed', success);

    if (finalMessage) {
        appendConsoleOutput('\n' + finalMessage);
    }

    if (success) {
        appendConsoleOutput('\n[OK] Command completed successfully', 'success');
    } else {
        appendConsoleOutput('\n[ERROR] Command failed', 'error');
    }

    // Refresh dashboard if on that view
    if (currentView === 'dashboard') {
        loadDashboard();
    }
}

function hideConsole() {
    // Close WebSocket if active
    if (activeWebSocket) {
        activeWebSocket.close();
        activeWebSocket = null;
    }
    $('#consoleModal').removeClass('flex').addClass('hidden');
}

function escapeHtml(unsafe) {
    return unsafe
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#039;");
}

// Close modals
$('#closeModal').on('click', hideModal);
$('#closeConsole, #closeConsoleBtn').on('click', hideConsole);

// WebSocket streaming execution
function executeWithWebSocket(operation, path, options = {}) {
    const displayName = operation.charAt(0).toUpperCase() + operation.slice(1);
    showConsole(`${displayName}: ${path}`);

    // Determine WebSocket URL
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/ws/execute`;

    try {
        activeWebSocket = new WebSocket(wsUrl);

        activeWebSocket.onopen = () => {
            updateConsoleStatus('Running...');
            appendConsoleOutput(`> ${operation} ${path}\n`, 'section');

            // Send operation request
            const request = {
                operation: operation,
                path: path,
                executor_args: options.executor_args || [],
                yes: options.yes || false
            };
            activeWebSocket.send(JSON.stringify(request));
        };

        activeWebSocket.onmessage = (event) => {
            try {
                const msg = JSON.parse(event.data);

                switch (msg.type) {
                    case 'start':
                        appendConsoleOutput(`Operation ID: ${msg.data.operation_id}`, 'info');
                        break;
                    case 'output':
                        appendConsoleOutput(msg.data.text);
                        break;
                    case 'complete':
                        finishConsole(msg.data.success);
                        activeWebSocket.close();
                        activeWebSocket = null;
                        break;
                    case 'error':
                        appendConsoleOutput(msg.data.message, 'error');
                        finishConsole(false);
                        activeWebSocket.close();
                        activeWebSocket = null;
                        break;
                }
            } catch (e) {
                appendConsoleOutput(event.data);
            }
        };

        activeWebSocket.onerror = (error) => {
            appendConsoleOutput('WebSocket error occurred', 'error');
            finishConsole(false, 'Connection error');
        };

        activeWebSocket.onclose = (event) => {
            if (event.code !== 1000) {
                finishConsole(false, 'Connection closed unexpectedly');
            }
        };
    } catch (e) {
        // Fallback to HTTP if WebSocket fails
        appendConsoleOutput('WebSocket not available, falling back to HTTP...', 'warning');
        executeWithHttp(operation, path, options);
    }
}

// HTTP fallback for execution
async function executeWithHttp(operation, path, options = {}) {
    try {
        updateConsoleStatus('Running...');
        appendConsoleOutput(`> ${operation} ${path}\n`, 'section');

        const requestBody = {
            path: path,
            executor_args: options.executor_args || []
        };

        if (operation === 'destroy') {
            requestBody.yes = options.yes || true;
        }

        const response = await $.ajax({
            url: `/api/${operation}`,
            method: 'POST',
            contentType: 'application/json',
            data: JSON.stringify(requestBody)
        });

        if (response.data) {
            appendConsoleOutput(response.data);
        }

        if (response.success) {
            finishConsole(true);
        } else {
            if (response.error) {
                appendConsoleOutput('\n' + response.error, 'error');
            }
            finishConsole(false);
        }
    } catch (error) {
        finishConsole(false, `Error: ${error.message}`);
    }
}

// Load infrastructure
async function loadInfrastructure() {
    try {
        const response = await $.get('/api/infrastructure');
        if (response.success && response.data) {
            infrastructure = response.data;
            $('#infraName').text(infrastructure.name);
            $('#infraPath').text(infrastructure.path);
            return true;
        } else {
            showStatus('Failed to load infrastructure: ' + (response.error || 'Unknown error'), 'error');
            return false;
        }
    } catch (error) {
        showStatus('Failed to load infrastructure: ' + error.message, 'error');
        return false;
    }
}

// Check template packs
async function checkTemplatePacks() {
    try {
        const response = await $.get('/api/template-packs');
        if (response.success && response.data && response.data.length > 0) {
            $('#templatePackWarning').addClass('hidden');
            return true;
        } else {
            $('#templatePackWarning').removeClass('hidden');
            return false;
        }
    } catch (error) {
        $('#templatePackWarning').removeClass('hidden');
        return false;
    }
}

// Load Dashboard
async function loadDashboard() {
    try {
        const response = await $.get('/api/dashboard');
        if (response.success && response.data) {
            const data = response.data;

            // Update stats
            $('#statProjectCount').text(data.project_count);
            $('#statEnvironmentCount').text(data.environments.length);
            $('#statKindCount').text(Object.keys(data.projects_by_kind).length);
            $('#statOperationCount').text(data.recent_operations.length);

            // Render kind distribution
            renderDistribution('#kindDistribution', data.projects_by_kind, data.project_count, 'indigo');

            // Render environment distribution
            renderDistribution('#envDistribution', data.projects_by_environment, data.project_count, 'green');

            // Render recent operations
            renderRecentOperations(data.recent_operations);
        }
    } catch (error) {
        console.error('Failed to load dashboard:', error);
    }
}

function renderDistribution(selector, data, total, color) {
    const $container = $(selector);
    $container.empty();

    if (Object.keys(data).length === 0) {
        $container.html('<p class="text-gray-500 text-sm">No data</p>');
        return;
    }

    const sortedEntries = Object.entries(data).sort((a, b) => b[1] - a[1]);

    sortedEntries.forEach(([name, count]) => {
        const percentage = total > 0 ? Math.round((count / total) * 100) : 0;
        $container.append(`
            <div class="flex items-center gap-2">
                <span class="text-sm text-gray-600 w-24 truncate" title="${name}">${name}</span>
                <div class="flex-1 bg-gray-200 rounded-full h-2">
                    <div class="bg-${color}-500 h-2 rounded-full" style="width: ${percentage}%"></div>
                </div>
                <span class="text-sm text-gray-500 w-12 text-right">${count}</span>
            </div>
        `);
    });
}

function renderRecentOperations(operations) {
    const $container = $('#recentOperations');
    $container.empty();

    if (operations.length === 0) {
        $container.html('<p class="text-gray-500 text-sm">No operations yet</p>');
        return;
    }

    operations.forEach(op => {
        const statusColor = op.status === 'completed' ? 'green' :
                          op.status === 'failed' ? 'red' :
                          op.status === 'running' ? 'yellow' : 'gray';
        const statusIcon = op.status === 'completed' ? '&#10003;' :
                          op.status === 'failed' ? '&#10007;' :
                          op.status === 'running' ? '&#8635;' : '&#8226;';

        const startTime = new Date(op.started_at).toLocaleString();

        $container.append(`
            <div class="flex items-center justify-between p-2 bg-gray-50 rounded hover:bg-gray-100">
                <div class="flex items-center gap-2">
                    <span class="text-${statusColor}-600 font-bold">${statusIcon}</span>
                    <span class="font-medium text-sm">${op.operation}</span>
                    <span class="text-gray-500 text-xs font-mono">${op.project_path}</span>
                </div>
                <div class="flex items-center gap-2">
                    <span class="text-xs px-2 py-1 rounded bg-${statusColor}-100 text-${statusColor}-700">${op.status}</span>
                    <span class="text-xs text-gray-400">${startTime}</span>
                </div>
            </div>
        `);
    });
}

// Load Graph
async function loadGraph() {
    try {
        showLoading();
        const response = await $.get('/api/graph');
        if (response.success && response.data) {
            $('#mermaidCode').text(response.data.mermaid);
        } else {
            $('#mermaidCode').text('// Failed to load graph: ' + (response.error || 'Unknown error'));
        }
    } catch (error) {
        $('#mermaidCode').text('// Failed to load graph: ' + error.message);
    } finally {
        hideLoading();
    }
}

// Load projects
async function loadProjects() {
    if (!infrastructure) {
        return;
    }

    showLoading();
    try {
        const params = new URLSearchParams({ path: infrastructure.path });
        const response = await $.get(`/api/projects?${params.toString()}`);

        if (response.success) {
            allProjects = response.data || [];
            renderProjects(allProjects);
            updateSearchResults(allProjects.length, allProjects.length);
        } else {
            showStatus(`Error loading projects: ${response.error}`, 'error');
        }
    } catch (error) {
        showStatus(`Failed to load projects: ${error.message}`, 'error');
    } finally {
        hideLoading();
    }
}

function updateSearchResults(visible, total) {
    $('#projectSearchResults').text(
        visible === total
            ? `Showing ${total} project(s)`
            : `Showing ${visible} of ${total} project(s)`
    );
}

function renderProjects(projectsToRender) {
    const $list = $('#projectsList');
    $list.empty();

    if (projectsToRender.length === 0) {
        $list.html('<p class="text-gray-500 text-center py-8">No projects found</p>');
        return;
    }

    projectsToRender.forEach((project, idx) => {
        const $card = $(`
            <div class="project-card border border-gray-200 rounded p-3 hover:shadow transition-shadow"
                 data-name="${project.name.toLowerCase()}"
                 data-kind="${project.kind.toLowerCase()}"
                 data-environments="${project.environments.join(',').toLowerCase()}"
                 data-path="${project.path.toLowerCase()}">
                <div class="flex items-center justify-between mb-2">
                    <h4 class="font-semibold text-sm">${project.name}</h4>
                    <span class="bg-blue-100 text-blue-800 px-2 py-0.5 rounded text-xs">${project.kind}</span>
                </div>
                <p class="text-xs text-gray-500 mb-2 font-mono">${project.path}</p>
                <div class="flex flex-wrap gap-1 mb-3">
                    ${project.environments.map(env => `
                        <span class="bg-green-100 text-green-700 px-2 py-0.5 rounded text-xs">${env}</span>
                    `).join('')}
                </div>
                <div class="flex gap-2 pt-2 border-t">
                    <button class="btn-preview px-2 py-1 text-xs bg-blue-500 text-white rounded hover:bg-blue-600" data-path="${project.path}" data-envs='${JSON.stringify(project.environments)}'>Preview</button>
                    <button class="btn-apply px-2 py-1 text-xs bg-green-500 text-white rounded hover:bg-green-600" data-path="${project.path}" data-envs='${JSON.stringify(project.environments)}'>Apply</button>
                    <button class="btn-refresh px-2 py-1 text-xs bg-yellow-500 text-white rounded hover:bg-yellow-600" data-path="${project.path}" data-envs='${JSON.stringify(project.environments)}'>Refresh</button>
                    <button class="btn-destroy px-2 py-1 text-xs bg-red-500 text-white rounded hover:bg-red-600" data-path="${project.path}" data-envs='${JSON.stringify(project.environments)}'>Destroy</button>
                </div>
            </div>
        `);
        $list.append($card);
    });

    // Attach event handlers
    $('.btn-preview').on('click', function() {
        const path = $(this).data('path');
        const envs = $(this).data('envs');
        selectEnvironmentAndExecute('preview', path, envs);
    });

    $('.btn-apply').on('click', function() {
        const path = $(this).data('path');
        const envs = $(this).data('envs');
        selectEnvironmentAndExecute('apply', path, envs);
    });

    $('.btn-refresh').on('click', function() {
        const path = $(this).data('path');
        const envs = $(this).data('envs');
        selectEnvironmentAndExecute('refresh', path, envs);
    });

    $('.btn-destroy').on('click', function() {
        const path = $(this).data('path');
        const envs = $(this).data('envs');
        if (confirm('Are you sure you want to destroy this project? This action cannot be undone.')) {
            selectEnvironmentAndExecute('destroy', path, envs, { yes: true });
        }
    });
}

function selectEnvironmentAndExecute(operation, projectPath, environments, options = {}) {
    let selectedEnv = null;

    if (!environments || environments.length === 0) {
        showStatus('No environments found for this project', 'error');
        return;
    } else if (environments.length === 1) {
        selectedEnv = environments[0];
    } else {
        const envList = environments.map((env, idx) => `${idx + 1}. ${env}`).join('\n');
        const selection = prompt(`Select environment:\n${envList}\n\nEnter number (1-${environments.length}):`);

        if (!selection) {
            return;
        }

        const envIndex = parseInt(selection) - 1;
        if (envIndex < 0 || envIndex >= environments.length) {
            showStatus('Invalid environment selection', 'error');
            return;
        }

        selectedEnv = environments[envIndex];
    }

    const envPath = `${projectPath}/environments/${selectedEnv}`;
    executeWithWebSocket(operation, envPath, options);
}

// Search functionality
$('#projectSearch').on('input', function() {
    const searchTerm = $(this).val().toLowerCase().trim();

    if (searchTerm === '') {
        renderProjects(allProjects);
        updateSearchResults(allProjects.length, allProjects.length);
        return;
    }

    const filtered = allProjects.filter(project => {
        return project.name.toLowerCase().includes(searchTerm) ||
               project.kind.toLowerCase().includes(searchTerm) ||
               project.environments.some(env => env.toLowerCase().includes(searchTerm)) ||
               project.path.toLowerCase().includes(searchTerm);
    });

    renderProjects(filtered);
    updateSearchResults(filtered.length, allProjects.length);
});

// Template packs and templates data
let templatePacks = [];
let currentTemplateInputs = [];

// Load template packs for create project form
async function loadTemplatePacks() {
    try {
        const response = await $.get('/api/template-packs');
        if (response.success && response.data) {
            templatePacks = response.data;
            populateTemplatePackSelect();
        }
    } catch (error) {
        console.error('Failed to load template packs:', error);
    }
}

function populateTemplatePackSelect() {
    const $select = $('#createTemplatePack');
    $select.empty().append('<option value="">Select a template pack...</option>');

    templatePacks.forEach(pack => {
        $select.append(`<option value="${pack.name}">${pack.name}${pack.description ? ' - ' + pack.description : ''}</option>`);
    });
}

// Handle template pack selection
$('#createTemplatePack').on('change', function() {
    const packName = $(this).val();
    const $templateSelect = $('#createTemplate');

    $templateSelect.empty().append('<option value="">Select a template...</option>');
    $('#createTemplateInputs').empty();
    currentTemplateInputs = [];

    if (!packName) {
        $templateSelect.prop('disabled', true);
        return;
    }

    const pack = templatePacks.find(p => p.name === packName);
    if (pack && pack.templates) {
        pack.templates.forEach(template => {
            $templateSelect.append(`<option value="${template.name}" data-kind="${template.kind}">${template.name}${template.description ? ' - ' + template.description : ''}</option>`);
        });
        $templateSelect.prop('disabled', false);
    }
});

// Handle template selection
$('#createTemplate').on('change', async function() {
    const packName = $('#createTemplatePack').val();
    const templateName = $(this).val();

    if (!packName || !templateName) {
        $('#createTemplateInputs').empty();
        currentTemplateInputs = [];
        return;
    }

    try {
        const response = await $.get(`/api/template-packs/${packName}/templates/${templateName}`);
        if (response.success && response.data) {
            currentTemplateInputs = response.data.inputs || [];
            renderTemplateInputs(currentTemplateInputs);

            const $envSelect = $('#createEnvironment');
            $envSelect.empty().append('<option value="">Select an environment...</option>');
            response.data.environments.forEach(env => {
                $envSelect.append(`<option value="${env}">${env}</option>`);
            });
        }
    } catch (error) {
        showStatus('Failed to load template details: ' + error.message, 'error');
    }
});

function renderTemplateInputs(inputs) {
    const $container = $('#createTemplateInputs');
    $container.empty();

    if (!inputs || inputs.length === 0) {
        $('#createTemplateInputsSection').addClass('hidden');
        return;
    }

    inputs.forEach(input => {
        const inputHtml = createInputField(input);
        $container.append(inputHtml);
    });

    if (inputs.length > 0) {
        $('#createTemplateInputsSection').removeClass('hidden');
    }
}

function createInputField(input) {
    const id = `input-${input.name}`;
    const required = input.required ? 'required' : '';
    const defaultValue = input.default !== undefined ? (typeof input.default === 'object' ? JSON.stringify(input.default) : input.default) : '';
    const description = input.description || '';

    let fieldHtml = '';

    switch (input.type) {
        case 'select':
            const options = (input.options || []).map(opt =>
                `<option value="${opt.value}" ${opt.value === defaultValue ? 'selected' : ''}>${opt.label}</option>`
            ).join('');
            fieldHtml = `
                <div class="mb-3">
                    <label class="block text-sm font-medium text-gray-700 mb-1" for="${id}">${input.name}</label>
                    <select id="${id}" name="${input.name}" class="w-full px-3 py-2 border border-gray-300 rounded text-sm" ${required}>
                        ${options}
                    </select>
                    ${description ? `<p class="text-xs text-gray-500 mt-1">${description}</p>` : ''}
                </div>`;
            break;

        case 'boolean':
            fieldHtml = `
                <div class="mb-3">
                    <label class="flex items-center text-sm">
                        <input type="checkbox" id="${id}" name="${input.name}" ${defaultValue === true || defaultValue === 'true' ? 'checked' : ''} class="mr-2">
                        <span class="font-medium text-gray-700">${input.name}</span>
                    </label>
                    ${description ? `<p class="text-xs text-gray-500 mt-1">${description}</p>` : ''}
                </div>`;
            break;

        case 'number':
            fieldHtml = `
                <div class="mb-3">
                    <label class="block text-sm font-medium text-gray-700 mb-1" for="${id}">${input.name}</label>
                    <input type="number" id="${id}" name="${input.name}" value="${defaultValue}"
                        ${input.min !== undefined ? `min="${input.min}"` : ''}
                        ${input.max !== undefined ? `max="${input.max}"` : ''}
                        class="w-full px-3 py-2 border border-gray-300 rounded text-sm" ${required}>
                    ${description ? `<p class="text-xs text-gray-500 mt-1">${description}</p>` : ''}
                </div>`;
            break;

        case 'password':
            fieldHtml = `
                <div class="mb-3">
                    <label class="block text-sm font-medium text-gray-700 mb-1" for="${id}">${input.name}</label>
                    <input type="password" id="${id}" name="${input.name}" value="${defaultValue}"
                        class="w-full px-3 py-2 border border-gray-300 rounded text-sm" ${required}>
                    ${description ? `<p class="text-xs text-gray-500 mt-1">${description}</p>` : ''}
                </div>`;
            break;

        default:
            fieldHtml = `
                <div class="mb-3">
                    <label class="block text-sm font-medium text-gray-700 mb-1" for="${id}">${input.name}</label>
                    <input type="text" id="${id}" name="${input.name}" value="${defaultValue}"
                        class="w-full px-3 py-2 border border-gray-300 rounded text-sm" ${required}>
                    ${description ? `<p class="text-xs text-gray-500 mt-1">${description}</p>` : ''}
                </div>`;
    }

    return fieldHtml;
}

// Handle create project form submission
$('#createProjectForm').on('submit', async function(e) {
    e.preventDefault();

    const templatePack = $('#createTemplatePack').val();
    const template = $('#createTemplate').val();
    const environment = $('#createEnvironment').val();
    const projectName = $('#createProjectName').val();
    const description = $('#createProjectDescription').val();

    if (!templatePack || !template || !environment || !projectName) {
        showStatus('Please fill in all required fields', 'error');
        return;
    }

    const inputs = {};
    currentTemplateInputs.forEach(input => {
        const $field = $(`#input-${input.name}`);
        if ($field.length) {
            if (input.type === 'boolean') {
                inputs[input.name] = $field.is(':checked');
            } else if (input.type === 'number') {
                inputs[input.name] = parseFloat($field.val()) || 0;
            } else {
                inputs[input.name] = $field.val();
            }
        }
    });

    showLoading();

    try {
        const response = await $.ajax({
            url: '/api/projects/create',
            method: 'POST',
            contentType: 'application/json',
            data: JSON.stringify({
                template_pack: templatePack,
                template: template,
                environment: environment,
                name: projectName,
                description: description || null,
                inputs: inputs
            })
        });

        if (response.success) {
            showStatus('Project created successfully!', 'success');
            $('#createProjectForm')[0].reset();
            $('#createTemplate').prop('disabled', true);
            $('#createTemplateInputs').empty();
            $('#createTemplateInputsSection').addClass('hidden');
            await loadProjects();
            loadDashboard();
        } else {
            showStatus('Failed to create project: ' + response.error, 'error');
        }
    } catch (error) {
        showStatus('Failed to create project: ' + error.message, 'error');
    } finally {
        hideLoading();
    }
});

// Initialize on page load
$(document).ready(async function() {
    // Load infrastructure (required)
    const infraLoaded = await loadInfrastructure();
    if (!infraLoaded) {
        showStatus('Failed to load infrastructure. Please restart the server from an infrastructure directory.', 'error');
        return;
    }

    // Check for template packs
    const hasTemplatePacks = await checkTemplatePacks();

    // Load projects
    await loadProjects();

    // Load template packs for create form
    if (hasTemplatePacks) {
        await loadTemplatePacks();
    }

    // Load dashboard
    await loadDashboard();
});
