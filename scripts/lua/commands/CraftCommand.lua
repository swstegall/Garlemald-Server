--[[

CraftJudge 

Operates the Crafting system.

Functions:

loadTextData()  
	Desc: Loads all gamesheets needed and instantiates a CraftJudge.
	Params: None

start(facility, requestsMode, material1, material2, material3, material4, material5, material6, material7, material8)
	Desc: Opens the Craft Start widget, with any preloaded materials. Widget has two modes; one for normal synthesis and another
		  for local leve "requested items" mode.
	Params:	* facility		- The current facility id buff the player may have.
			* requestMode 	- If true, switches the UI to Requested Items mode otherwise it opens Normal Synthesis mode.
			* material1-8	- ItemID for each of the 8 material slots. If empty, they must be set to 0 or the client will crash.
		
closeCraftStartWidget()
	Desc: Closes the Craft Start widget.
	Params: None

selectRcp(itemId)
	Desc: 	Selects the recipe to be crafted. May be a legacy function but still required to properly initialize the UI. Requires start() to have
			been called.
	Params:	* itemId 		- The itemID of the item to be crafted.
	
confirmRcp(craftedItem, quantity, crystalItem1, crystalQuantity1, crystalQuantity1, crystalItem2, crystalQuantity2, recommendedSkill, recommendedFacility)
	Desc: Opens the confirmation window, detailing what is needed and the item that will be created. Requires a selectRcp() call first.
	Params:	* craftedItem			- The itemID of the item to be crafted.
			* quantity				- Quantity of crafted items.
			* crystalItem1	 		- The first required crystal itemID for crafting.
			* crystalQuantity1		- Quantity of the first crystal.
			* crystalItem2			- The second required crystal itemID for crafting.
			* crystalQuantity2		- Quantity of the second crystal.
			* recommendedSkill		- Which itemID to display under the "Recommended Skill" panel.
			* recommendedFacility	- Which facility to display under the "Recommended Facility" panel.

selectCraftQuest()
	Desc: Opens the journal to select the local leve that the player would like to do.
	Params: None

askContinueLocalLeve(localLeveID, craftedItem, itemsCompleted, craftTotal, attempts)
	Desc: Opens the dialog to continue crafting for a local leve after an item was completed.
	Params: * localLeveID			- The id of the current leve in progress.
			* craftedItem			- The current crafted item id.
			* itemsCompleted		- Number of items crafted so far.
			* craftTotal			- Number of items to be crafted in total.
			* attempts				- The number of attempts left.

askRetryLocalleve(localLeveID, allowanceCount)
	Desc: Opens the dialog to retry the local leve (at the expense of an allowance) if the player had failed it.
	Params: * localLeveID			- The failed level id.
			* allowanceCount		- How many allowances the player has.

openCraftProgressWidget(durability, quality, hqChance)
	Desc: Opens the crafting minigame, sets starting values.
	Params: * durability			- Durability of the current item.
			* quality				- Starting quality of the current item.
			* hqChance				- Starting chance to get a HQ item.

craftCommandUI(classID, hasWait, command1, command2, command3, command4, command5)
	Desc: Sets the available command list and waits for the player to select a command.
	Params:
			* classID				- The current crafting class. Must be set properly to show the three synthesis commands.
			* hasWait				- If true, adds the wait command.
			* command1-5			- Five possible crafting commands (crafting skills).

craftTuningUI(command1, command2, command3, command4, command5, command6, command7, command8)
	Desc: Displays a full list of commands for the legacy "Tuning" phase that happens after crafting. Deprecated in 1.23b.
	Params: * command1-8			- The list of commands available.

updateInfo(progress, durability, quality, tuningItem, tuningItemQuality, tuningItemQuantity, hqChance)
	Desc: Updates the progress UI components and text boxes.
	Params: * progress				- The current crafting progress percentage. Value is from 0 to 100.
			* durability			- The current durability of the crafted item.
			* quality				- The current quality of the crafted item.
			* tuningItem			- The crafted item to show in the Tuning UI. Nil if crafting. Deprecated in 1.23b.
			* tuningItemQuality 	- The quality of the item to show in the Tuning UI. Nil if crafting. Deprecated in 1.23b.
			* tuningItemQuantity	- The amount of the item to show in the Tuning UI. Nil if crafting. Deprecated in 1.23b.
			* hqChance				- The current chance of an HQ craft.

closeCraftProgressWidget()
	Desc: Closes the crafting minigame widget.
	Params: None
	
cfmQst()
	Desc: Quest confirmation window for when starting a crafting quest from the journal.
	Params:

confirmLeve()
	Desc: Opens the summery page for the local leve.
	Params: * localLeveID			- The quest id of the leve you are confirming.
			* difficulty			- Changes the objective.
			* craftedItem?			-
			* ?						-
			* numSuccess			- The number of successful crafts you did.
			* remainingMaterials	- The number of materials you have left.
			* hasMaterials			- Shows the in-progress panel of successes and attempts left.
			* ?						-

startRepair(craftMode, item, quality, durability, hasMateria, spiritbind)
	Desc: Opens the repair item widget.
	Params: * craftMode				- Either 0 or 1. Anything else crashes.
			* item					- ItemID of the item to be repaired.
			* quality				- Quality of the item to be repaired.
			* durability			- Durability of the item to be repaired.
			* hasMateria			- Shows an icon if the item to be repaired has materia attached.
			* spiritbind			- Spiritbind of the item to be repaired.

askJoinMateria()
displayRate()

askJoinResult(isSuccess, item, itemQuality, materia, materiaNumber, isSpiritBound)
	Desc: Opens the result widget after materia melding is done.
	Params: * isSuccess				- True if the meld was successful.
			* item					- Item ID of the melded item.
			* quality				- Quality of the melded item.
			* materia				- Item ID of the materia being melded.
			* materiaNumber			- Total count of materia on the item.
			* isSpiritBound			- True if the item is spiritbound. Causes icon to appear.
	
Notes:

Class ID + Starting skill
 29 CRP = 22550
 30 BSM = 22556
 31 ARM = 22562
 32 GSM = 22568
 33 LTW = 22574
 34 WVR = 22580
 35 ALC = 22586
 36 CUL = 22592

Leve objectives/rewards are in passiveGL_craft.

* Index 1: 
* Index 2: Recommended Class
* Index 3: Issuing Authority
* Index 7: Levequest Location
* Index 8: Deliver Display Name
* Starts at index 14. Four sections for the four difficulties.
* Required Item, Amount, ?, Recommended Level, , Reward Item, Reward Amount, |

--]]

require ("global")

local skillAnim = {
    [22553] = 0x10002000;
    [22554] = 0x10001000;
    [22555] = 0x10003000;
    [29531] = 0x10009002;
}

local craftStartWidgetOpen = false;

function onEventStarted(player, commandactor, triggerName, arg1, arg2, arg3, arg4, checkedActorId)
    local MENU_CANCEL, MENU_MAINHAND, MENU_OFFHAND, MENU_REQUEST = 0, 1, 2, 3;
    local MENU_RECENT, MENU_AWARDED, MENU_RECENT_DETAILED, MENU_AWARDED_DETAILED = 7, 8, 9, 10;
    
    local debugMessage = true;

    local isRecipeRecentSent = false;
    local isRecipeAwardSent = false;
	
	local craftJudge = GetStaticActor("CraftJudge");
    local recipeResolver = GetRecipeResolver();	

	local operationResult;
	local operationMode = -1;
	local recipeMode = -1;
	local chosenMaterials;
	
	local facilityId = 0;
    local isRequestedItemsMode = false;  -- False = The default state.  True = User picked a quest recipe/local leve
	local recentRecipes;
	local awardedRecipes;
	
    local currentCraftQuest = nil;  -- Use this to store any chosen craft quest
	local currentCraftQuestGuildleve = nil;  -- Use this to store any chosen local leve
    	
	callClientFunction(player, "delegateCommand", craftJudge, "loadTextData", commandactor);      

	player:ChangeState(30); 
	
	while operationMode ~= 0 do
        
		-- Operate the start crafting window... confusing shit
		if (craftStartWidgetOpen == false) then
			-- Shows the initial window 
			local startMats = {0, 0, 0, 0, 0, 0, 0, 0};
			if (isRequestedItemsMode == true) then -- If requested items, preload the quest recipe materials
				startMats = recipeResolver.RecipeToMatIdTable(currentCraftQuestGuildleve.getRecipe());
			end
			operationResult = {callClientFunction(player, "delegateCommand", craftJudge, "start", commandactor, facilityId, isRequestedItemsMode, unpack(startMats))};
			craftStartWidgetOpen = true;
		elseif ((operationMode == MENU_RECENT or operationMode == MENU_AWARDED) and recipeMode ~= 0) then
			local prepedMaterials;
			-- Recent Recipes/Awarded Recipes
			if (operationMode == MENU_RECENT) then
				prepedMaterials = recipeResolver.RecipeToMatIdTable(recentRecipes[recipeMode]);
			else 			
				prepedMaterials = recipeResolver.RecipeToMatIdTable(awardedRecipes[recipeMode]);
			end
			-- Causes the item info window to appear for recent/awarded recipes. Only happens if a recipe was chosen.
			operationResult = {callClientFunction(player, "delegateCommand", craftJudge, "start", commandactor, -2, isRequestedItemsMode, unpack(prepedMaterials))};
		else
			-- Keep window going if the user "returned" to the starting point
			operationResult = {callClientFunction(player, "delegateCommand", craftJudge, "start", commandactor, -1, isRequestedItemsMode)};
        end
		
		operationMode = operationResult[1];
		recipeMode = operationResult[2];
				
        if debugMessage then player:SendMessage(0x20, "", "[DEBUG] Menu ID: " .. tostring(operationMode) .. ", RecipeMode : " .. recipeMode); end      
		
		-- Operation 
        if operationMode == MENU_CANCEL then 
            closeCraftStartWidget(player, craftJudge, commandactor);
        elseif (operationMode == MENU_MAINHAND or operationMode == MENU_OFFHAND) then 
            -- Recipe choosing loop
			while (true) do			
				-- Figure out the number of preloaded mats
				local numArgs = #operationResult;
				local numMatArgs = numArgs - 2;
				local materials;
				player:SendMessage(0x20, "", "[DEBUG] " .. tostring(numArgs));				
				player:SendMessage(0x20, "", "[DEBUG] " .. tostring(numMatArgs));
				
				-- Handle the possible args returned: Either 0 player items, 1 player item, 2+ palyer items. The rest is always the remaining prepped items.
				if (numMatArgs == 8 and type(operationResult[3]) == "number") then
					materials = {unpack(operationResult, 3)};
				elseif (numMatArgs == 8 and type(operationResult[3]) ~= "number") then
					player:SendMessage(0x20, "", "[DEBUG] " .. tostring(player:GetItemPackage(operationResult[3].itemPackage):GetItemAtSlot(operationResult[3].slot).itemId));
					materials = {player:GetItemPackage(operationResult[3].itemPackage):GetItemAtSlot(operationResult[3].slot).itemId, unpack(operationResult, 3)};
				else
					local itemIds = {};
					for i=0,operationResult[3].itemSlots.length do
						converted = player:GetItemPackage(operationResult[3].itemPackages[i]):GetItemAtSlot(operationResult[3].slots[i]).itemId
					end
					materials = {unpack(itemIds), unpack(operationResult, 4)};
				end				
				
				-- Choosing a recipe from the given materials
				local recipes = recipeResolver.GetRecipeFromMats(unpack(materials));				
				local itemIds = recipeResolver.RecipesToItemIdTable(recipes);
				
				-- No recipes found
				if (#itemIds == 0) then
					player:SendGameMessage(GetWorldMaster(), 40201, 0x20); -- You cannot synthesize with those materials.
					break;
				end
				
				local chosenRecipeIndex = callClientFunction(player, "delegateCommand", craftJudge, "selectRcp", commandactor, unpack(itemIds));
				
				-- Hit back on recipe list
				if (chosenRecipeIndex <= 0) then break end;
					
				chosenRecipe = recipes[chosenRecipeIndex-1];
				
				if (chosenRecipe ~= nil) then                          
					-- Player confirms recipe
					local recipeConfirmed = callClientFunction(player, "delegateCommand", craftJudge, "confirmRcp", commandactor, 
						chosenRecipe.resultItemID, 
						chosenRecipe.resultQuantity, 
						chosenRecipe.crystalId1, 
						chosenRecipe.crystalQuantity1, 
						chosenRecipe.crystalId2, 
						chosenRecipe.crystalQuantity2, 
						0, 
						0); 

					if recipeConfirmed then
						closeCraftStartWidget(player, craftJudge, commandactor);
						isRecipeRecentSent = false;
						isRecipeAwardSent = false;
						
						-- CRAFTING STARTED
						currentlyCrafting = startCrafting(player, commandactor, craftJudge, operationMode, chosenRecipe, currentCraftQuestGuildleve, 80, 100, 50); 
						
						--Once crafting is over, return to the original non-quest state.
						isRequestedItemsMode = false;
						currentCraftQuestGuildleve = nil;  
						currentCraftQuest = nil;           
						
						break;
					end
				end
			end
			-- End of Recipe choosing loops
        elseif operationMode == MENU_REQUEST then -- Conditional button label based on isRequestedItemsMode 
			closeCraftStartWidget(player, craftJudge, commandactor);
				
            if isRequestedItemsMode == false then    -- "Request Items" hit, close Start and open up the Quest select                
                isRecipeRecentSent = false;
                isRecipeAwardSent = false;                    
                
                local quest = getCraftQuest(player, craftJudge, commandactor);
				if (quest ~= nil) then
					isRequestedItemsMode = true;
					if (quest.isCraftPassiveGuildleve()) then
						currentCraftQuestGuildleve = quest;
					else
						currentCraftQuest = quest;
					end
				end
            elseif isRequestedItemsMode == true then -- "Normal Synthesis" button hit   
                isRequestedItemsMode = false;
                currentCraftQuestGuildleve = nil;  
                currentCraftQuest = nil;
            end        
        elseif operationMode == MENU_RECENT then -- "Recipes" button hit
            if isRecipeRecentSent == false then
				recentRecipes = player.GetRecentRecipes();
				local itemIds = recipeResolver.RecipesToItemIdTable(recentRecipes);
                callClientFunction(player, "delegateCommand", craftJudge, "selectRcp", commandactor, unpack(itemIds)); -- Load up recipe list
                isRecipeRecentSent = true;
            end
        elseif operationMode == MENU_AWARDED then -- "Awarded Recipes" tab hit  
            if isRecipeAwardSent == false then
				awardedRecipes = player.GetAwardedRecipes();
				local itemIds = recipeResolver.RecipesToItemIdTable(awardedRecipes);
                callClientFunction(player, "delegateCommand", craftJudge, "selectRcp", commandactor, unpack(itemIds)); -- Load up Award list
                isRecipeAwardSent = true;
            end
        elseif ((operationMode == MENU_RECENT_DETAILED or operationMode == MENU_AWARDED_DETAILED) and recipeMode > 0) then -- Pop-up for an item's stats/craft mats on a recent recipe			
			local chosenRecipe = operationMode == MENU_RECENT_DETAILED and recentRecipes[recipeMode-1] or recentRecipes[awardedMode-1];
			local recipeConfirmed = callClientFunction(player, "delegateCommand", craftJudge, "confirmRcp", commandactor, 
				chosenRecipe.resultItemID, 
				chosenRecipe.resultQuantity, 
				chosenRecipe.crystalId1, 
				chosenRecipe.crystalQuantity1, 
				chosenRecipe.crystalId2, 
				chosenRecipe.crystalQuantity2, 
				0, 
				0);
				
			-- This should never call? The window with this button only appears when you select a recent recipe with not enough materials. Otherwise it just auto-fills your "table".
			if (recipeConfirmed) then
				closeCraftStartWidget(player, craftJudge, commandactor);
				isRecipeRecentSent = false;
				isRecipeAwardSent = false;
				currentlyCrafting = startCrafting(player, commandactor, craftJudge, operationMode, chosenRecipe, isRequestedItemsMode, 80, 100, 50);
			end
        else
            break;
        end
    end	
	
    player:ResetMusic();
    player:ChangeState(0);
    player:EndEvent();	
end

-- Handles the menus to pick a crafter quest or local leve quest that run separate widgets from the Start command.
-- Returns whether a quest was selected, and what id the quest is.
function getCraftQuest(player, craftJudge, commandactor);
    local questId = nil;	
	
	while (true) do
		local questCommandId = callClientFunction(player, "delegateCommand", craftJudge, "selectCraftQuest", commandactor);
		
		if questCommandId then
			questId = questCommandId - 0xA0F00000;
			
			-- Craft Quest Chosen
			if isCraftQuest(questId) then
				local quest = player.GetQuest(questId);			
				local confirm = callClientFunction(player, "delegateCommand", craftJudge, "cfmQst", commandactor, quest.getQuestId(), 20, 1, 1, 1, 0, 0, "<Path Companion>");				
				if confirm == true then
					player:SendGameMessage(craftJudge, 21, 0x20);
					return quest;
				end
			-- PassiveGL Quest Chosen
			elseif isLocalLeve(questId) then
				local difficulty = 0;
				local hasMaterials = 1;
				
				local quest = player:getQuestGuildleve(questId);
				
				if (quest ~= nil) then
					-- Did they pickup the materials?
					if (quest:hasMaterials() == false) then
						player:SendGameMessage(GetWorldMaster(), 40210, 0x20); -- You have not obtained the proper materials from the client.						
					-- Did they use em all up?
					elseif (quest:getRemainingMaterials() == 0) then
						player:SendGameMessage(GetWorldMaster(), 40211, 0x20); -- You have used up all of the provided materials.			
					-- Confirm dialog
					else
						local confirm = callClientFunction(player, "delegateCommand", craftJudge, "confirmLeve", commandactor, 
							quest:getQuestId(),
							quest:getCurrentDifficulty() + 1, -- Lua, 1-indexed
							0, 
							quest:getCurrentCrafted(),
							quest:getRemainingMaterials(),
							quest:hasMaterials() and 1 or 0, -- Fucked up way of doing terneries on Lua 
							0
						);

						-- Quest confirmed
						if (confirm == true) then
							return quest;
						end	
					end
				else
					return nil; -- Shouldn't happen unless db fucked with
				end
			-- Scenario Quest Chosen
			else
				-- TEMP for now. Cannot find source for what happens if you confirm a non-craft quest.
			   player:SendGameMessage(GetWorldMaster(), 40209, 0x20); -- You cannot undertake that endeavor.
			end
		else
			return nil;
		end
	end
end

function isScenarioQuest(id)
    if (id >= 110001 and id <= 120026) then
        return true;
    else
        return false;
    end
end


function isCraftQuest(id)
    if (id >= 110300 and id <= 110505) then
        return true;
    else
        return false;
    end
end


function isLocalLeve(id)
    if (id >= 120001 and id <= 120452) then
        return true;
    else
        return false;
    end
end

function closeCraftStartWidget(player, craftJudge, commandactor)
	callClientFunction(player, "delegateCommand", craftJudge, "closeCraftStartWidget", commandactor);
	craftStartWidgetOpen = false;
end

-- No real logic in this function.  Just smoke and mirrors to 'see' the minigame in action at the minimum level.
function startCrafting(player, commandactor, craftJudge, hand, recipe, quest, startDur, startQly, startHQ)
    
    local worldMaster = GetWorldMaster();
    local progress = 0;
    local attempts = 5;
    local craftedCount = 0;
    local craftTotal = 2;
    
    player:ChangeState(30+hand);  -- Craft kneeling w/ appropriate tool out
    player:ChangeMusic(73);
    callClientFunction(player, "delegateCommand", craftJudge, "openCraftProgressWidget", commandactor, startDur, startQly, startHQ); 

    while (true) do     
        local progDiff = math.random(30,50);
        local duraDiff = math.random(1,3);
        local qltyDiff = math.random(0,2);

        if (progress >= 100) then            
            callClientFunction(player, "delegateCommand", craftJudge, "closeCraftProgressWidget", commandactor);
            
			-- Handle local levequest craft success
            if quest then	
				quest:craftSuccess();
				
				if (quest:getCurrentCrafted() >= quest:getObjectiveQuantity()) then
					attentionMessage(player, 40121, quest:getQuestId(), quest:getCurrentCrafted(), quest:getObjectiveQuantity()); -- "All items for <QuestId> complete!"
				else
					attentionMessage(player, 40119, quest:getQuestId(), quest:getCurrentCrafted(), quest:getObjectiveQuantity()); -- "<QuestId> Successfull. (<crafted> of <attempts>)"
				end
			
				-- Continue local levequest  (should this be in here??)
				if (quest:getRemainingMaterials() ~= 0) then
					continueLeve = callClientFunction(player, "delegateCommand", craftJudge, "askContinueLocalleve", commandactor,
						quest:getQuestId(),
						quest:getRecipe().resultItemID, 
						quest:getCurrentCrafted(),
						quest:getObjectiveQuantity(), 
						quest:getRemainingMaterials()
					);

					if (continueLeve == 1) then
						progress = 0;
						callClientFunction(player, "delegateCommand", craftJudge, "openCraftProgressWidget", commandactor, startDur, startQly, startHQ);
					else
						break;
					end
				else
					break;
				end				
			-- Normal synth craft success
            else                
				player:SendGameMessage(GetWorldMaster(), 40111, 0x20, player, recipe.resultItemID, 1, recipe.resultQuantity);  -- "You create <#3 quantity> <#1 item> <#2 quality>."				
				player:getItemPackage(location):addItem(recipe.resultItemID, recipe.resultQuantity, 1);
				break;
            end
        end		
        
        choice = callClientFunction(player, "delegateCommand", craftJudge, "craftCommandUI", commandactor, 29, 2, 29530,29531,29532,29533,29534);
        --player:SendMessage(0x20, "", "[DEBUG] Command id selected: "..choice);
                
        if (choice) then
            
            if skillAnim[choice] then
                player:PlayAnimation(skillAnim[choice]);
            end
            
            wait(3);

            player:SendGameMessage(worldMaster, 40108, 0x20, choice,2);
            
            if (choice ~= 29531) then
                progress = progress + progDiff;
                
                if (progress >= 100) then 
                    progress = 100;
                end
                
                startDur = startDur - duraDiff;
                startQly = startQly + qltyDiff;
           
                player:SendGameMessage(worldMaster, 40102, 0x20, progDiff);
                player:SendGameMessage(worldMaster, 40103, 0x20, duraDiff);
                player:SendGameMessage(worldMaster, 40104, 0x20, qltyDiff);
            end
                                                                                                          --prg  dur  qly, ???, ???, ???,   HQ
            callClientFunction(player, "delegateCommand", craftJudge, "updateInfo", commandactor, progress, startDur, startQly, nil, nil, nil, nil, nil);
            
            --testChoice = callClientFunction(player, "delegateCommand", craftJudge, "craftTuningUI", commandactor, 29501, 24233, 29501,29501, 24223, 29501,12008,12004);
        end
    end
end