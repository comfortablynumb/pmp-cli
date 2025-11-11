// Global state
let templatePacks = [];
let projects = [];
let infrastructure = null;

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
    $('#projectActionModal').removeClass('hidden').addClass('flex');
}

function hideModal() {
    $('#projectActionModal').removeClass('flex').addClass('hidden');
}

// Tab switching
$('.tab-button').on('click', function() {
    const tab = $(this).data('tab');

    // Update tab buttons
    $('.tab-button').removeClass('border-blue-600 text-blue-600').addClass('text-gray-600');
    $(this).removeClass('text-gray-600').addClass('border-blue-600 text-blue-600');

    // Update tab content
    $('.tab-content').addClass('hidden');
    $(`#${tab}-tab`).removeClass('hidden');
});

// Close modal
$('#closeModal').on('click', hideModal);

// Templates Tab
$('#loadTemplatesBtn').on('click', async function() {
    showLoading();
    try {
        const response = await $.get('/api/template-packs');
        if (response.success) {
            templatePacks = response.data;
            renderTemplatePacks(templatePacks);
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

function renderTemplatePacks(packs) {
    const $list = $('#templatePacksList');
    $list.empty();

    if (packs.length === 0) {
        $list.html('<p class="text-gray-500">No template packs found</p>');
        return;
    }

    packs.forEach(pack => {
        const $packCard = $(`
            <div class="border border-gray-200 rounded-lg p-4 hover:shadow-md transition-shadow">
                <h3 class="text-xl font-semibold mb-2">${pack.name}</h3>
                ${pack.description ? `<p class="text-gray-600 mb-3">${pack.description}</p>` : ''}
                <div class="flex items-center justify-between">
                    <span class="text-sm text-gray-500">${pack.templates.length} template(s)</span>
                    <button class="view-templates-btn bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700"
                            data-pack="${pack.name}">
                        View Templates
                    </button>
                </div>
            </div>
        `);
        $list.append($packCard);
    });

    // Attach event handlers
    $('.view-templates-btn').on('click', function() {
        const packName = $(this).data('pack');
        const pack = templatePacks.find(p => p.name === packName);
        if (pack) {
            showTemplatesModal(pack);
        }
    });
}

function showTemplatesModal(pack) {
    let content = `<h4 class="font-semibold mb-3">Templates in ${pack.name}</h4>`;

    if (pack.templates.length === 0) {
        content += '<p class="text-gray-500">No templates found in this pack</p>';
    } else {
        content += '<div class="space-y-3">';
        pack.templates.forEach(template => {
            content += `
                <div class="border border-gray-200 rounded p-3">
                    <h5 class="font-semibold">${template.name}</h5>
                    ${template.description ? `<p class="text-sm text-gray-600 mt-1">${template.description}</p>` : ''}
                    <div class="mt-2 text-sm">
                        <span class="bg-blue-100 text-blue-800 px-2 py-1 rounded mr-2">Kind: ${template.kind}</span>
                        <span class="bg-gray-100 text-gray-800 px-2 py-1 rounded mr-2">API: ${template.api_version}</span>
                        ${template.environments.length > 0 ? `<span class="bg-green-100 text-green-800 px-2 py-1 rounded">Environments: ${template.environments.join(', ')}</span>` : ''}
                    </div>
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
                </div>
            `;
        });
        content += '</div>';
    }

    showModal(`Template Pack: ${pack.name}`, content);
}

// Projects Tab
$('#loadProjectsBtn').on('click', loadProjects);
$('#projectNameFilter, #projectKindFilter').on('input', loadProjects);

async function loadProjects() {
    showLoading();
    try {
        const nameFilter = $('#projectNameFilter').val();
        const kindFilter = $('#projectKindFilter').val();

        const params = new URLSearchParams();
        if (nameFilter) params.append('name', nameFilter);
        if (kindFilter) params.append('kind', kindFilter);

        const response = await $.get(`/api/projects?${params.toString()}`);
        if (response.success) {
            projects = response.data;
            renderProjects(projects);
            showStatus('Projects loaded successfully', 'success');
        } else {
            showStatus(`Error: ${response.error}`, 'error');
        }
    } catch (error) {
        showStatus(`Failed to load projects: ${error.message}`, 'error');
    } finally {
        hideLoading();
    }
}

function renderProjects(projects) {
    const $list = $('#projectsList');
    $list.empty();

    if (projects.length === 0) {
        $list.html('<p class="text-gray-500">No projects found</p>');
        return;
    }

    projects.forEach(project => {
        const $projectCard = $(`
            <div class="border border-gray-200 rounded-lg p-4 hover:shadow-md transition-shadow">
                <div class="flex justify-between items-start mb-2">
                    <div>
                        <h3 class="text-xl font-semibold">${project.name}</h3>
                        ${project.description ? `<p class="text-gray-600 mt-1">${project.description}</p>` : ''}
                    </div>
                    <span class="bg-purple-100 text-purple-800 px-3 py-1 rounded text-sm">${project.kind}</span>
                </div>
                <div class="text-sm text-gray-500 mb-3">
                    <strong>Path:</strong> ${project.path}
                </div>
                ${project.environments.length > 0 ? `
                    <div class="mb-3">
                        <strong class="text-sm">Environments:</strong>
                        <div class="flex flex-wrap gap-2 mt-1">
                            ${project.environments.map(env => `
                                <span class="bg-green-100 text-green-800 px-2 py-1 rounded text-sm">${env}</span>
                            `).join('')}
                        </div>
                    </div>
                ` : ''}
                <div class="flex gap-2">
                    <button class="preview-btn bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700"
                            data-path="${project.path}">
                        Preview
                    </button>
                    <button class="apply-btn bg-green-600 text-white px-3 py-1 rounded text-sm hover:bg-green-700"
                            data-path="${project.path}">
                        Apply
                    </button>
                    <button class="refresh-btn bg-yellow-600 text-white px-3 py-1 rounded text-sm hover:bg-yellow-700"
                            data-path="${project.path}">
                        Refresh
                    </button>
                    <button class="destroy-btn bg-red-600 text-white px-3 py-1 rounded text-sm hover:bg-red-700"
                            data-path="${project.path}">
                        Destroy
                    </button>
                </div>
            </div>
        `);
        $list.append($projectCard);
    });

    // Attach event handlers
    $('.preview-btn').on('click', function() {
        const path = $(this).data('path');
        executeProjectAction('preview', path);
    });

    $('.apply-btn').on('click', function() {
        const path = $(this).data('path');
        if (confirm('Are you sure you want to apply changes to this project?')) {
            executeProjectAction('apply', path);
        }
    });

    $('.refresh-btn').on('click', function() {
        const path = $(this).data('path');
        executeProjectAction('refresh', path);
    });

    $('.destroy-btn').on('click', function() {
        const path = $(this).data('path');
        if (confirm('WARNING: This will destroy all resources! Are you sure?')) {
            executeProjectAction('destroy', path, true);
        }
    });
}

async function executeProjectAction(action, path, confirmed = false) {
    showLoading();
    try {
        const payload = {
            path: path,
            executor_args: []
        };

        if (action === 'destroy') {
            payload.yes = confirmed;
        }

        const response = await $.ajax({
            url: `/api/${action}`,
            method: 'POST',
            contentType: 'application/json',
            data: JSON.stringify(payload)
        });

        if (response.success) {
            showStatus(`${action.charAt(0).toUpperCase() + action.slice(1)} completed successfully`, 'success');
        } else {
            showStatus(`Error: ${response.error}`, 'error');
        }
    } catch (error) {
        showStatus(`Failed to execute ${action}: ${error.responseJSON?.error || error.message}`, 'error');
    } finally {
        hideLoading();
    }
}

// Generate Tab
$('#genTemplatePack').on('change', async function() {
    const packName = $(this).val();
    if (!packName) {
        $('#genTemplate').prop('disabled', true).html('<option value="">Select a template...</option>');
        return;
    }

    showLoading();
    try {
        const response = await $.get(`/api/template-packs/${packName}/templates`);
        if (response.success) {
            const templates = response.data;
            let options = '<option value="">Select a template...</option>';
            templates.forEach(template => {
                options += `<option value="${template.name}">${template.name}</option>`;
            });
            $('#genTemplate').prop('disabled', false).html(options);
        } else {
            showStatus(`Error loading templates: ${response.error}`, 'error');
        }
    } catch (error) {
        showStatus(`Failed to load templates: ${error.message}`, 'error');
    } finally {
        hideLoading();
    }
});

$('#genTemplate').on('change', async function() {
    const packName = $('#genTemplatePack').val();
    const templateName = $(this).val();

    if (!packName || !templateName) {
        $('#genEnvironmentDiv').addClass('hidden');
        $('#genInputsDiv').empty();
        return;
    }

    showLoading();
    try {
        const response = await $.get(`/api/template-packs/${packName}/templates/${templateName}`);
        if (response.success) {
            const template = response.data;

            // Handle environments
            if (template.environments && template.environments.length > 0) {
                $('#genEnvironmentDiv').removeClass('hidden');
                let envOptions = '<option value="">None</option>';
                template.environments.forEach(env => {
                    envOptions += `<option value="${env}">${env}</option>`;
                });
                $('#genEnvironment').html(envOptions);
            } else {
                $('#genEnvironmentDiv').addClass('hidden');
            }

            // Render inputs
            renderGenerateInputs(template.inputs);
        } else {
            showStatus(`Error loading template details: ${response.error}`, 'error');
        }
    } catch (error) {
        showStatus(`Failed to load template details: ${error.message}`, 'error');
    } finally {
        hideLoading();
    }
});

function renderGenerateInputs(inputs) {
    const $div = $('#genInputsDiv');
    $div.empty();

    if (inputs.length === 0) {
        return;
    }

    $div.append('<h3 class="font-semibold text-lg mt-4 mb-2">Template Inputs</h3>');

    inputs.forEach(input => {
        if (input.name === 'name' || input.name === '_name') {
            return; // Skip name inputs as we have a dedicated field
        }

        let inputHtml = `<div><label class="block text-sm font-medium text-gray-700 mb-2">${input.name}`;
        if (input.description) {
            inputHtml += ` <span class="text-gray-500 font-normal">(${input.description})</span>`;
        }
        inputHtml += '</label>';

        if (input.enum_values && input.enum_values.length > 0) {
            // Select input
            inputHtml += `<select name="${input.name}" class="w-full border border-gray-300 rounded px-4 py-2">`;
            input.enum_values.forEach(value => {
                const selected = input.default === value ? 'selected' : '';
                inputHtml += `<option value="${value}" ${selected}>${value}</option>`;
            });
            inputHtml += '</select>';
        } else if (typeof input.default === 'boolean') {
            // Checkbox input
            const checked = input.default ? 'checked' : '';
            inputHtml += `<input type="checkbox" name="${input.name}" class="rounded" ${checked}>`;
        } else {
            // Text/number input
            const value = input.default !== null && input.default !== undefined ? input.default : '';
            const type = typeof input.default === 'number' ? 'number' : 'text';
            inputHtml += `<input type="${type}" name="${input.name}" value="${value}" class="w-full border border-gray-300 rounded px-4 py-2">`;
        }

        inputHtml += '</div>';
        $div.append(inputHtml);
    });
}

$('#generateForm').on('submit', async function(e) {
    e.preventDefault();

    const packName = $('#genTemplatePack').val();
    const templateName = $('#genTemplate').val();
    const environment = $('#genEnvironment').val();
    const name = $('#genName').val();
    const outputDir = $('#genOutputDir').val();

    // Collect inputs
    const inputs = { name: name };
    $('#genInputsDiv input, #genInputsDiv select').each(function() {
        const $input = $(this);
        const inputName = $input.attr('name');
        if (inputName) {
            if ($input.attr('type') === 'checkbox') {
                inputs[inputName] = $input.is(':checked');
            } else if ($input.attr('type') === 'number') {
                inputs[inputName] = parseFloat($input.val()) || 0;
            } else {
                inputs[inputName] = $input.val();
            }
        }
    });

    const payload = {
        template_pack: packName,
        template: templateName,
        environment: environment || null,
        name: name,
        inputs: inputs,
        output_dir: outputDir || null
    };

    showLoading();
    try {
        const response = await $.ajax({
            url: '/api/generate',
            method: 'POST',
            contentType: 'application/json',
            data: JSON.stringify(payload)
        });

        if (response.success) {
            showStatus('Files generated successfully!', 'success');
            $('#generateForm')[0].reset();
            $('#genTemplate').prop('disabled', true);
            $('#genEnvironmentDiv').addClass('hidden');
            $('#genInputsDiv').empty();
        } else {
            showStatus(`Error: ${response.error}`, 'error');
        }
    } catch (error) {
        showStatus(`Failed to generate files: ${error.responseJSON?.error || error.message}`, 'error');
    } finally {
        hideLoading();
    }
});

// Infrastructure Tab
$('#loadInfrastructureBtn').on('click', async function() {
    showLoading();
    try {
        const response = await $.get('/api/infrastructure');
        if (response.success) {
            infrastructure = response.data;
            renderInfrastructure(infrastructure);
            showStatus('Infrastructure loaded successfully', 'success');
        } else {
            showStatus(`Error: ${response.error}`, 'error');
        }
    } catch (error) {
        showStatus(`Failed to load infrastructure: ${error.message}`, 'error');
    } finally {
        hideLoading();
    }
});

function renderInfrastructure(infra) {
    const $div = $('#infrastructureDetails');
    $div.empty();

    let html = `
        <div class="border border-gray-200 rounded-lg p-4">
            <h3 class="text-xl font-semibold mb-2">${infra.name}</h3>
            ${infra.description ? `<p class="text-gray-600 mb-4">${infra.description}</p>` : ''}

            <div class="mb-4">
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

    $div.html(html);
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

// Load template packs for the generate form on page load
$(document).ready(async function() {
    try {
        const response = await $.get('/api/template-packs');
        if (response.success) {
            const packs = response.data;
            let options = '<option value="">Select a template pack...</option>';
            packs.forEach(pack => {
                options += `<option value="${pack.name}">${pack.name}</option>`;
            });
            $('#genTemplatePack').html(options);
        }
    } catch (error) {
        console.error('Failed to load template packs for generate form:', error);
    }
});
