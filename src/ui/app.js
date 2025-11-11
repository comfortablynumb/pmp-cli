// Global state
let templatePacks = [];
let infrastructures = [];
let currentInfrastructure = null;
let currentTemplatePack = null;
let currentPickerPath = null;
let pickerCallback = null;

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

// Directory Picker
function openDirectoryPicker(callback, initialPath = null) {
    pickerCallback = callback;
    currentPickerPath = initialPath;
    loadDirectoryEntries(currentPickerPath);
    $('#directoryPickerModal').removeClass('hidden').addClass('flex');
}

function closeDirectoryPicker() {
    $('#directoryPickerModal').removeClass('flex').addClass('hidden');
    pickerCallback = null;
    currentPickerPath = null;
}

async function loadDirectoryEntries(path) {
    try {
        const response = await $.post('/api/browse', { path: path });

        if (response.success) {
            currentPickerPath = path || response.data[0]?.path || '.';
            $('#currentPickerPath').text(currentPickerPath);
            renderDirectoryEntries(response.data);
        } else {
            showStatus(`Error browsing directory: ${response.error}`, 'error');
        }
    } catch (error) {
        showStatus(`Failed to browse directory: ${error.message}`, 'error');
    }
}

function renderDirectoryEntries(entries) {
    const $content = $('#directoryPickerContent');
    $content.empty();

    if (entries.length === 0) {
        $content.html('<p class="text-gray-500 text-center py-8">No directories found</p>');
        return;
    }

    entries.forEach(entry => {
        const $entry = $(`
            <div class="directory-entry flex items-center gap-3 p-3 border border-gray-200 rounded-lg hover:bg-indigo-50 cursor-pointer transition-colors"
                 data-path="${entry.path}">
                <svg class="w-6 h-6 text-indigo-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                          d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"></path>
                </svg>
                <span class="flex-1 font-medium text-gray-700">${entry.name}</span>
                <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"></path>
                </svg>
            </div>
        `);

        $entry.on('click', function() {
            const path = $(this).data('path');
            loadDirectoryEntries(path);
        });

        $content.append($entry);
    });
}

$('#closeDirectoryPicker, #cancelDirectoryPicker').on('click', closeDirectoryPicker);

$('#selectCurrentDirectory').on('click', function() {
    if (pickerCallback && currentPickerPath) {
        pickerCallback(currentPickerPath);
        closeDirectoryPicker();
    }
});

// Tab switching
$('.tab-button').on('click', function() {
    const tab = $(this).data('tab');

    // Update tab buttons
    $('.tab-button').removeClass('border-indigo-600 text-indigo-600').addClass('text-gray-600');
    $(this).removeClass('text-gray-600').addClass('border-indigo-600 text-indigo-600');

    // Update tab content
    $('.tab-content').addClass('hidden');
    $(`#${tab}-tab`).removeClass('hidden');
});

// Close modal
$('#closeModal').on('click', hideModal);

// Infrastructure Tab - Browse button
$('#browseInfraBtn').on('click', function() {
    openDirectoryPicker(function(path) {
        $('#infraPath').val(path);
    });
});

// Infrastructure Tab - Load button
$('#addInfraBtn').on('click', async function() {
    const path = $('#infraPath').val().trim() || '.';
    showLoading();

    try {
        const response = await $.post('/api/infrastructure/load', {
            path: path
        });

        if (response.success) {
            // Add to infrastructures list
            infrastructures.push(response.data);
            currentInfrastructure = response.data;
            renderInfrastructures();
            updateCurrentContext();
            $('#infraPath').val('');
            showStatus(`Successfully loaded infrastructure: ${response.data.name}`, 'success');
        } else {
            showStatus(`Error: ${response.error}`, 'error');
        }
    } catch (error) {
        showStatus(`Failed to load infrastructure: ${error.message}`, 'error');
    } finally {
        hideLoading();
    }
});

// Template Packs Tab - Installation method toggle
$('#installMethod').on('change', function() {
    const method = $(this).val();
    if (method === 'git') {
        $('#gitInstallDiv').removeClass('hidden');
        $('#localInstallDiv').addClass('hidden');
    } else {
        $('#gitInstallDiv').addClass('hidden');
        $('#localInstallDiv').removeClass('hidden');
    }
});

// Template Packs - Browse local button
$('#browseLocalBtn').on('click', function() {
    openDirectoryPicker(function(path) {
        $('#localPath').val(path);
    });
});

// Template Packs - Git installation
$('#installGitBtn').on('click', async function() {
    const gitUrl = $('#gitUrl').val().trim();
    if (!gitUrl) {
        showStatus('Please enter a Git URL', 'warning');
        return;
    }

    showLoading();
    try {
        const response = await $.post('/api/template-packs/install-git', {
            git_url: gitUrl
        });

        if (response.success) {
            $('#gitUrl').val('');
            showStatus(response.data, 'success');
            // Automatically refresh the template packs list
            $('#loadTemplatePacksBtn').trigger('click');
        } else {
            showStatus(`Error: ${response.error}`, 'error');
        }
    } catch (error) {
        showStatus(`Failed to clone repository: ${error.message}`, 'error');
    } finally {
        hideLoading();
    }
});

// Template Packs - Local installation
$('#installLocalBtn').on('click', async function() {
    const localPath = $('#localPath').val().trim();
    if (!localPath) {
        showStatus('Please enter a local path', 'warning');
        return;
    }

    showLoading();
    try {
        const response = await $.post('/api/template-packs/install-local', {
            local_path: localPath
        });

        if (response.success) {
            $('#localPath').val('');
            showStatus(response.data, 'success');
            showStatus(response.data + ' (Use this path in template_packs_paths to access)', 'info');
        } else {
            showStatus(`Error: ${response.error}`, 'error');
        }
    } catch (error) {
        showStatus(`Failed to load template pack: ${error.message}`, 'error');
    } finally {
        hideLoading();
    }
});

// Load Template Packs
$('#loadTemplatePacksBtn').on('click', async function() {
    showLoading();
    try {
        const response = await $.get('/api/template-packs');
        if (response.success) {
            templatePacks = response.data;
            renderTemplatePacks();
            showStatus('Template packs loaded successfully', 'success');
        } else {
            showStatus(`Error: ${response.error}`, 'error');
        }
    } catch (error) {
        showStatus(`Failed to load template packs: ${error.message}`, 'error');
    } finally {
        hideLoading();
    }
});

function updateCurrentContext() {
    let context = [];
    if (currentInfrastructure) {
        context.push(`Infrastructure: ${currentInfrastructure.name}`);
    }
    if (currentTemplatePack) {
        context.push(`Template Pack: ${currentTemplatePack.name}`);
    }
    $('#currentContext').text(context.join(' | ') || 'No context');
}

function renderInfrastructures() {
    const $list = $('#infrastructureList');
    $list.empty();

    if (infrastructures.length === 0) {
        $list.html('<p class="text-gray-500">No infrastructures loaded. Use the form above to load an infrastructure.</p>');
        return;
    }

    infrastructures.forEach((infra, index) => {
        const isCurrent = currentInfrastructure && currentInfrastructure.name === infra.name;
        const $infraCard = $(`
            <div class="border ${isCurrent ? 'border-indigo-400 bg-indigo-50' : 'border-gray-200'} rounded-lg p-4">
                <div class="flex items-center justify-between mb-2">
                    <h3 class="text-xl font-semibold text-indigo-700">${infra.name}</h3>
                    ${isCurrent ? '<span class="bg-indigo-600 text-white px-2 py-1 rounded text-xs">Active</span>' : ''}
                </div>
                ${infra.description ? `<p class="text-gray-600 mb-3">${infra.description}</p>` : ''}
                <div class="text-sm text-gray-500 mb-3">
                    <div class="flex items-center gap-2">
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                                  d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"></path>
                        </svg>
                        <span class="font-mono text-xs">${infra.path}</span>
                    </div>
                    <div class="mt-1">${infra.environments.length} environment(s), ${infra.categories.length} ${infra.categories.length === 1 ? 'category' : 'categories'}</div>
                </div>
                <div class="flex gap-2 flex-wrap">
                    ${!isCurrent ? `<button class="select-infra-btn bg-indigo-600 text-white px-3 py-1 rounded text-sm hover:bg-indigo-700" data-index="${index}">Use This</button>` : ''}
                    <button class="view-infra-btn bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700" data-index="${index}">View Details</button>
                    <button class="view-projects-btn bg-green-600 text-white px-3 py-1 rounded text-sm hover:bg-green-700" data-index="${index}">View Projects</button>
                    <button class="remove-infra-btn bg-red-600 text-white px-3 py-1 rounded text-sm hover:bg-red-700" data-index="${index}">Remove</button>
                </div>
            </div>
        `);
        $list.append($infraCard);
    });

    // Attach event handlers
    $('.select-infra-btn').on('click', function() {
        const index = parseInt($(this).data('index'));
        currentInfrastructure = infrastructures[index];
        renderInfrastructures();
        updateCurrentContext();
        showStatus(`Switched to infrastructure: ${currentInfrastructure.name}`, 'success');
    });

    $('.view-infra-btn').on('click', function() {
        const index = parseInt($(this).data('index'));
        const infra = infrastructures[index];
        showInfrastructureDetails(infra);
    });

    $('.view-projects-btn').on('click', async function() {
        const index = parseInt($(this).data('index'));
        const infra = infrastructures[index];
        await showProjects(infra);
    });

    $('.remove-infra-btn').on('click', function() {
        const index = parseInt($(this).data('index'));
        const infra = infrastructures[index];
        if (confirm(`Remove infrastructure "${infra.name}"?`)) {
            infrastructures.splice(index, 1);
            if (currentInfrastructure && currentInfrastructure.name === infra.name) {
                currentInfrastructure = infrastructures.length > 0 ? infrastructures[0] : null;
            }
            renderInfrastructures();
            updateCurrentContext();
            showStatus(`Removed infrastructure: ${infra.name}`, 'success');
        }
    });
}

function showInfrastructureDetails(infra) {
    let content = `
        <div class="space-y-4">
            ${infra.description ? `<p class="text-gray-700">${infra.description}</p>` : ''}

            <div>
                <h4 class="font-semibold mb-2">Path</h4>
                <p class="font-mono text-sm text-gray-600 bg-gray-100 p-2 rounded">${infra.path}</p>
            </div>

            <div>
                <h4 class="font-semibold mb-2">Environments</h4>
                <div class="flex flex-wrap gap-2">
                    ${infra.environments.map(env => `
                        <span class="bg-green-100 text-green-800 px-3 py-1 rounded">${env}</span>
                    `).join('')}
                </div>
            </div>

            ${infra.categories && infra.categories.length > 0 ? `
                <div>
                    <h4 class="font-semibold mb-2">Categories</h4>
                    <div class="space-y-2">
                        ${renderCategories(infra.categories)}
                    </div>
                </div>
            ` : ''}
        </div>
    `;
    showModal(`Infrastructure: ${infra.name}`, content);
}

function renderCategories(categories, level = 0) {
    let html = '';
    categories.forEach(category => {
        const indent = level * 20;
        html += `
            <div style="margin-left: ${indent}px" class="border-l-2 border-blue-300 pl-3 py-2">
                <h5 class="font-semibold">${category.name}</h5>
                ${category.description ? `<p class="text-sm text-gray-600">${category.description}</p>` : ''}
                ${category.templates && category.templates.length > 0 ? `
                    <div class="mt-2 flex flex-wrap gap-2">
                        ${category.templates.map(t => `
                            <span class="bg-blue-100 text-blue-800 px-2 py-1 rounded text-xs">${t.template_pack}/${t.template}</span>
                        `).join('')}
                    </div>
                ` : ''}
                ${category.subcategories && category.subcategories.length > 0 ? renderCategories(category.subcategories, level + 1) : ''}
            </div>
        `;
    });
    return html;
}

async function showProjects(infra) {
    showLoading();
    try {
        const params = new URLSearchParams({ path: infra.path });
        const response = await $.get(`/api/projects?${params.toString()}`);

        if (response.success) {
            const projects = response.data;

            let content = `
                <div class="space-y-4">
                    <p class="text-gray-600">Projects in infrastructure: <strong>${infra.name}</strong></p>
                    <p class="text-sm text-gray-500 font-mono">${infra.path}</p>

                    ${projects.length === 0 ? `
                        <p class="text-gray-500 text-center py-8">No projects found in this infrastructure</p>
                    ` : `
                        <div class="space-y-3">
                            ${projects.map(project => `
                                <div class="border border-gray-200 rounded-lg p-4 hover:shadow-md transition-shadow">
                                    <div class="flex items-center justify-between mb-2">
                                        <h4 class="font-semibold text-lg">${project.name}</h4>
                                        <span class="bg-blue-100 text-blue-800 px-2 py-1 rounded text-xs">${project.kind}</span>
                                    </div>
                                    <p class="text-sm text-gray-600 mb-2">${project.path}</p>
                                    <div class="flex flex-wrap gap-1">
                                        ${project.environments.map(env => `
                                            <span class="bg-green-100 text-green-700 px-2 py-1 rounded text-xs">${env}</span>
                                        `).join('')}
                                    </div>
                                </div>
                            `).join('')}
                        </div>
                    `}
                </div>
            `;
            showModal(`Projects in ${infra.name}`, content);
        } else {
            showStatus(`Error loading projects: ${response.error}`, 'error');
        }
    } catch (error) {
        showStatus(`Failed to load projects: ${error.message}`, 'error');
    } finally {
        hideLoading();
    }
}

function renderTemplatePacks() {
    const $list = $('#templatePacksList');
    $list.empty();

    if (templatePacks.length === 0) {
        $list.html('<p class="text-gray-500">No template packs found. Install one using the form above or click Refresh.</p>');
        return;
    }

    templatePacks.forEach((pack, index) => {
        const isCurrent = currentTemplatePack && currentTemplatePack.name === pack.name;
        const $packCard = $(`
            <div class="border ${isCurrent ? 'border-purple-400 bg-purple-50' : 'border-gray-200'} rounded-lg p-4">
                <div class="flex items-center justify-between mb-2">
                    <h3 class="text-xl font-semibold text-purple-700">${pack.name}</h3>
                    ${isCurrent ? '<span class="bg-purple-600 text-white px-2 py-1 rounded text-xs">Active</span>' : ''}
                </div>
                ${pack.description ? `<p class="text-gray-600 mb-3">${pack.description}</p>` : ''}
                <div class="flex items-center justify-between">
                    <span class="text-sm text-gray-500">${pack.templates.length} template(s)</span>
                    <div class="flex gap-2">
                        <button class="view-templates-btn bg-purple-600 text-white px-3 py-1 rounded text-sm hover:bg-purple-700"
                                data-index="${index}">
                            View Templates & Generate
                        </button>
                    </div>
                </div>
            </div>
        `);
        $list.append($packCard);
    });

    // Attach event handlers
    $('.view-templates-btn').on('click', function() {
        const index = parseInt($(this).data('index'));
        const pack = templatePacks[index];
        currentTemplatePack = pack;
        updateCurrentContext();
        showTemplatesAndGenerate(pack);
    });
}

function showTemplatesAndGenerate(pack) {
    let content = `
        <div class="space-y-4">
            <p class="text-gray-700">${pack.description || ''}</p>

            <div class="bg-purple-50 p-4 rounded-lg border border-purple-200">
                <h4 class="font-semibold text-purple-800 mb-2">Templates in this pack (${pack.templates.length})</h4>
                ${pack.templates.length === 0 ? `
                    <p class="text-gray-500">No templates found in this pack</p>
                ` : `
                    <div class="space-y-3">
                        ${pack.templates.map(template => `
                            <div class="bg-white border border-gray-200 rounded p-3">
                                <div class="flex items-center justify-between mb-2">
                                    <h5 class="font-semibold">${template.name}</h5>
                                    <span class="bg-blue-100 text-blue-800 px-2 py-1 rounded text-xs">${template.kind}</span>
                                </div>
                                ${template.description ? `<p class="text-sm text-gray-600 mb-2">${template.description}</p>` : ''}
                                <div class="text-xs text-gray-500 mb-2">API: ${template.api_version}</div>
                                ${template.environments.length > 0 ? `
                                    <div class="text-xs text-gray-600 mb-2">
                                        Environments: ${template.environments.join(', ')}
                                    </div>
                                ` : ''}
                                ${template.inputs.length > 0 ? `
                                    <details class="mt-2">
                                        <summary class="text-sm text-blue-600 cursor-pointer">View Inputs (${template.inputs.length})</summary>
                                        <ul class="mt-2 ml-4 text-sm space-y-1">
                                            ${template.inputs.map(input => `
                                                <li>
                                                    <strong>${input.name}</strong>
                                                    ${input.description ? `: ${input.description}` : ''}
                                                    ${input.default !== null && input.default !== undefined ? ` (default: ${JSON.stringify(input.default)})` : ''}
                                                </li>
                                            `).join('')}
                                        </ul>
                                    </details>
                                ` : ''}
                                <div class="mt-3">
                                    <button class="generate-btn bg-green-600 text-white px-4 py-2 rounded text-sm hover:bg-green-700"
                                            data-pack="${pack.name}" data-template="${template.name}">
                                        Generate from this Template
                                    </button>
                                </div>
                            </div>
                        `).join('')}
                    </div>
                `}
            </div>
        </div>
    `;

    showModal(`Template Pack: ${pack.name}`, content);

    // Attach event handlers for generate buttons
    $('.generate-btn').on('click', async function() {
        const packName = $(this).data('pack');
        const templateName = $(this).data('template');
        await generateFromTemplate(packName, templateName);
    });
}

async function generateFromTemplate(packName, templateName) {
    const outputDir = prompt(`Enter output directory for generated files:`, './output');
    if (!outputDir) return;

    hideModal();
    showLoading();

    try {
        const response = await $.post('/api/generate', {
            template_pack: packName,
            template: templateName,
            environment: null,
            name: 'generated',
            inputs: {},
            output_dir: outputDir
        });

        if (response.success) {
            showStatus(`Successfully generated files to ${outputDir}`, 'success');
        } else {
            showStatus(`Error: ${response.error}`, 'error');
        }
    } catch (error) {
        showStatus(`Failed to generate: ${error.message}`, 'error');
    } finally {
        hideLoading();
    }
}

// Initialize on page load
$(document).ready(async function() {
    // Initialize infrastructure list
    renderInfrastructures();
    updateCurrentContext();

    // Load template packs automatically
    $('#loadTemplatePacksBtn').trigger('click');
});
