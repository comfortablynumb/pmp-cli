// Global state
let infrastructure = null;
let projects = [];
let allProjects = []; // For search

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

// Console Modal
function showConsole(title, initialMessage = '') {
    $('#consoleTitle').text(title);
    $('#consoleOutput').html(initialMessage || '<span class="text-gray-500">Waiting for output...</span>');
    $('#consoleSpinner').removeClass('hidden');
    $('#consoleActions').addClass('hidden');
    $('#consoleModal').removeClass('hidden').addClass('flex');
}

function appendConsoleOutput(message) {
    const $output = $('#consoleOutput');
    const currentHtml = $output.html();

    // Remove "Waiting for output..." message if present
    if (currentHtml.includes('Waiting for output')) {
        $output.html('');
    }

    $output.append(`<div>${escapeHtml(message)}</div>`);
    // Auto-scroll to bottom
    if ($output.length > 0 && $output[0]) {
        $output[0].scrollTop = $output[0].scrollHeight;
    }
}

function finishConsole(success, finalMessage = null) {
    $('#consoleSpinner').addClass('hidden');
    $('#consoleActions').removeClass('hidden');

    if (finalMessage) {
        appendConsoleOutput('\n' + finalMessage);
    }

    if (success) {
        appendConsoleOutput('\n✓ Command completed successfully');
    } else {
        appendConsoleOutput('\n✗ Command failed');
    }
}

function hideConsole() {
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
            <div class="project-card border border-gray-200 rounded p-2 hover:shadow transition-shadow"
                 data-name="${project.name.toLowerCase()}"
                 data-kind="${project.kind.toLowerCase()}"
                 data-environments="${project.environments.join(',').toLowerCase()}"
                 data-path="${project.path.toLowerCase()}">
                <div class="flex items-center justify-between mb-1">
                    <h4 class="font-semibold text-sm">${project.name}</h4>
                    <span class="bg-blue-100 text-blue-800 px-1.5 py-0.5 rounded text-xs">${project.kind}</span>
                </div>
                <p class="text-xs text-gray-500 mb-1 font-mono">${project.path}</p>
                <div class="flex flex-wrap gap-1 mb-2">
                    ${project.environments.map(env => `
                        <span class="bg-green-100 text-green-700 px-1.5 py-0.5 rounded text-xs">${env}</span>
                    `).join('')}
                </div>
                <div class="flex flex-wrap gap-2">
                    <button class="project-preview-btn bg-blue-500 text-white px-2 py-1 rounded text-xs hover:bg-blue-600"
                            data-path="${project.path}" data-environments='${JSON.stringify(project.environments)}'>
                        Preview
                    </button>
                    <button class="project-apply-btn bg-green-600 text-white px-2 py-1 rounded text-xs hover:bg-green-700"
                            data-path="${project.path}" data-environments='${JSON.stringify(project.environments)}'>
                        Apply
                    </button>
                    <button class="project-refresh-btn bg-yellow-600 text-white px-2 py-1 rounded text-xs hover:bg-yellow-700"
                            data-path="${project.path}" data-environments='${JSON.stringify(project.environments)}'>
                        Refresh
                    </button>
                    <button class="project-destroy-btn bg-red-600 text-white px-2 py-1 rounded text-xs hover:bg-red-700"
                            data-path="${project.path}" data-environments='${JSON.stringify(project.environments)}'>
                        Destroy
                    </button>
                </div>
            </div>
        `);
        $list.append($card);
    });

    // Attach event handlers
    $('.project-preview-btn').on('click', async function() {
        const path = $(this).data('path');
        const environments = JSON.parse($(this).attr('data-environments') || '[]');
        await executeProjectCommand('preview', path, environments, 'Preview');
    });

    $('.project-apply-btn').on('click', async function() {
        const path = $(this).data('path');
        const environments = JSON.parse($(this).attr('data-environments') || '[]');
        if (confirm('Apply changes to this project?')) {
            await executeProjectCommand('apply', path, environments, 'Apply');
        }
    });

    $('.project-refresh-btn').on('click', async function() {
        const path = $(this).data('path');
        const environments = JSON.parse($(this).attr('data-environments') || '[]');
        await executeProjectCommand('refresh', path, environments, 'Refresh');
    });

    $('.project-destroy-btn').on('click', async function() {
        const path = $(this).data('path');
        const environments = JSON.parse($(this).attr('data-environments') || '[]');
        if (confirm('⚠️ WARNING: This will destroy all resources in this project. Are you sure?')) {
            await executeProjectCommand('destroy', path, environments, 'Destroy');
        }
    });
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

// Execute project command
async function executeProjectCommand(command, projectPath, environments, displayName) {
    // Select environment
    let selectedEnv = null;

    if (!environments || environments.length === 0) {
        showStatus('No environments found for this project', 'error');
        return;
    } else if (environments.length === 1) {
        // Auto-select if only one environment
        selectedEnv = environments[0];
    } else {
        // Prompt user to select environment
        const envList = environments.map((env, idx) => `${idx + 1}. ${env}`).join('\n');
        const selection = prompt(`Select environment:\n${envList}\n\nEnter number (1-${environments.length}):`);

        if (!selection) {
            return; // User cancelled
        }

        const envIndex = parseInt(selection) - 1;
        if (envIndex < 0 || envIndex >= environments.length) {
            showStatus('Invalid environment selection', 'error');
            return;
        }

        selectedEnv = environments[envIndex];
    }

    // Build environment path
    const envPath = `${projectPath}/environments/${selectedEnv}`;

    showConsole(`${displayName}: ${envPath}`, `Executing ${command}...`);

    try {
        let endpoint = `/api/${command}`;
        let requestBody = { path: envPath, executor_args: [] };

        // For destroy, add yes flag
        if (command === 'destroy') {
            requestBody.yes = true;
        }

        appendConsoleOutput(`> ${command} ${envPath}\n`);

        const response = await $.post(endpoint, requestBody);

        if (response.success) {
            finishConsole(true, response.data || `${displayName} completed`);
        } else {
            finishConsole(false, response.error || `${displayName} failed`);
        }
    } catch (error) {
        finishConsole(false, `Error: ${error.message}`);
    }
}

// Create project button
$('#createProjectBtn').on('click', function() {
    showStatus('Project creation UI coming soon...', 'info');
    // TODO: Implement project creation modal
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
    await checkTemplatePacks();

    // Load projects
    await loadProjects();
});
